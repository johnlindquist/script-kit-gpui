#!/usr/bin/env bun
/// <reference types="bun-types" />
/**
 * NN=27 Notes-window actions chaos battery.
 *
 * Hidden/protocol-first only: no screenshots, native input, clipboard, or
 * frontmost-window claim. Each row writes a fail-closed JSON receipt under
 * .test-output/chaos-27-notes-window-actions.
 *
 * Rows:
 *   action-menu-races   rapid open/dismiss and idempotent open-while-opening
 *   hostile-formatting  Zalgo/RTL/ZWJ/multibyte/huge selections, round-trip
 *   enter-semantics     actionable, structural, and zero-match row behavior
 *   escape-ladder       popup dismiss -> editor focus -> honest Notes close
 *   autosave-race       formatting action while autosave debounce is pending
 *   popup-key-storm     navigation storms preserve a valid selection/execution
 */

import { existsSync, mkdirSync, readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { AttachedDriver, Driver, type Json } from "../devtools/driver";

const PROJECT_ROOT = resolve(import.meta.dir, "../..");
const OUTPUT_DIR = resolve(
	PROJECT_ROOT,
	process.env.PROBE_OUTPUT_DIR ?? ".test-output/chaos-27-notes-window-actions",
);
const BINARY = resolve(
	PROJECT_ROOT,
	process.env.PROBE_BINARY ??
		process.env.SCRIPT_KIT_GPUI_BINARY ??
		"target-agent/artifacts/chaos-27-notes-window-actions/script-kit-gpui",
);
const RUN_ID = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
const ATTACH_SESSION = process.env.PROBE_SESSION?.trim() || null;
const ATTACH_HOME = process.env.PROBE_HOME?.trim()
	? resolve(process.env.PROBE_HOME)
	: null;
const SCREEN_CLAIMED = process.env.PROBE_SCREEN_CLAIMED === "1";
const NOTES_TARGET: Json = { type: "kind", kind: "notes", index: 0 };
const ACTIONS_TARGET: Json = { type: "kind", kind: "actionsDialog" };
const ALL_ROWS = [
	"action-menu-races",
	"hostile-formatting",
	"enter-semantics",
	"escape-ladder",
	"autosave-race",
	"popup-key-storm",
] as const;
type RowId = (typeof ALL_ROWS)[number];
type CheckKind = "product" | "harness" | "environment";
type Check = { name: string; ok: boolean; kind: CheckKind; detail: Json };
type RowReceipt = {
	schemaVersion: 1;
	rowId: RowId;
	classification: string;
	checks: Check[];
	productFindings: Json[];
	harnessFindings: Json[];
	environmentFindings: Json[];
	evidence: Json;
	startedAt: string;
	finishedAt?: string;
};

class HarnessInvalid extends Error {}
class EnvironmentBlocked extends Error {}

mkdirSync(OUTPUT_DIR, { recursive: true });

function selectedRows(): RowId[] {
	const index = process.argv.indexOf("--rows");
	if (index < 0) return [...ALL_ROWS];
	const requested = [
		...new Set(
			String(process.argv[index + 1] ?? "")
				.split(",")
				.map((row) => row.trim())
				.filter(Boolean),
		),
	];
	const unknown = requested.filter((row) => !ALL_ROWS.includes(row as RowId));
	if (unknown.length > 0) {
		throw new HarnessInvalid(`unknown --rows values: ${unknown.join(", ")}`);
	}
	const rows = requested as RowId[];
	if (rows.length === 0) throw new HarnessInvalid("--rows selected no rows");
	return rows;
}

function walk(node: unknown, out: Json[] = []): Json[] {
	if (!node || typeof node !== "object") return out;
	if (Array.isArray(node)) {
		for (const child of node) walk(child, out);
		return out;
	}
	const json = node as Json;
	if (typeof json.semanticId === "string" || typeof json.id === "string")
		out.push(json);
	for (const value of Object.values(json)) walk(value, out);
	return out;
}

function visibleActions(dialog: Json | null): Json[] {
	if (!dialog) return [];
	if (Array.isArray(dialog.visibleActions)) return dialog.visibleActions;
	const sample = dialog.actions?.visibleSample;
	return Array.isArray(sample) ? sample : [];
}

function actionId(row: Json): string {
	return String(row.actionId ?? row.id ?? row.value ?? "");
}

function selectedActionId(dialog: Json | null): string | null {
	const value = dialog?.selection?.actionId ?? dialog?.selectedActionId;
	return typeof value === "string" && value.length > 0 ? value : null;
}

function fnv1a64(value: string): string {
	let hash = 0xcbf29ce484222325n;
	const prime = 0x100000001b3n;
	const mask = 0xffffffffffffffffn;
	for (const byte of new TextEncoder().encode(value)) {
		hash ^= BigInt(byte);
		hash = (hash * prime) & mask;
	}
	return `fnv1a64:${hash.toString(16).padStart(16, "0")}`;
}

function classifyLaunchError(error: unknown): never {
	const message = error instanceof Error ? error.message : String(error);
	if (
		/did not become ready|rpc.*timeout|timed out|operation not permitted|sandbox/i.test(
			message,
		)
	) {
		throw new EnvironmentBlocked(message);
	}
	throw new HarnessInvalid(message);
}

async function poll<T>(
	label: string,
	probe: () => Promise<T>,
	accept: (value: T) => boolean,
	timeoutMs = 7000,
): Promise<T> {
	const started = performance.now();
	let last: T | undefined;
	let lastError: unknown;
	while (performance.now() - started < timeoutMs) {
		try {
			last = await probe();
			if (accept(last)) return last;
		} catch (error) {
			lastError = error;
		}
		await Bun.sleep(25);
	}
	throw new HarnessInvalid(
		`${label} did not become observable in ${timeoutMs}ms; last=${JSON.stringify(last)} error=${String(lastError ?? "")}`,
	);
}

async function writeJson(path: string, value: unknown): Promise<void> {
	await Bun.write(path, `${JSON.stringify(value, null, 2)}\n`);
}

async function writeReceipt(receipt: RowReceipt): Promise<void> {
	const rowDir = join(OUTPUT_DIR, receipt.rowId);
	mkdirSync(rowDir, { recursive: true });
	await Promise.all([
		writeJson(join(OUTPUT_DIR, `${receipt.rowId}.json`), receipt),
		writeJson(join(rowDir, "receipt.json"), receipt),
	]);
}

async function runBattery() {
	const rows = selectedRows();
	const summary: Json = {
		schemaVersion: 1,
		tool: "chaos-notes-window-actions-probe",
		nn: 27,
		runId: RUN_ID,
		binary: BINARY,
		outputDir: OUTPUT_DIR,
		executor: ATTACH_SESSION
			? `Driver.attach session=${ATTACH_SESSION} hidden/protocol-first`
			: "Driver.launch sandboxHome hidden/protocol-first",
		screen: {
			claimed: SCREEN_CLAIMED,
			screenshots: false,
			nativeInput: false,
			ownerQueue: "manager round-90 fixer-of31 runtime cell",
		},
		requestedRows: rows,
		rowReceipts: [],
		productFindings: [],
		harnessFindings: [],
		environmentFindings: [],
	};

	let driver: Driver | AttachedDriver;
	try {
		if (ATTACH_SESSION) {
			if (!ATTACH_HOME || !ATTACH_HOME.startsWith("/tmp/")) {
				throw new HarnessInvalid(
					"PROBE_SESSION requires PROBE_HOME under /tmp so Notes mutations cannot reach the real HOME",
				);
			}
			driver = await Driver.attach({
				session: ATTACH_SESSION,
				defaultTimeoutMs: 10_000,
			});
			const attachedLog = join(driver.sessionDir, "app.log");
			const expectedKitPath = join(ATTACH_HOME, ".scriptkit");
			const log = existsSync(attachedLog)
				? readFileSync(attachedLog, "utf8")
				: "";
			if (!log.includes(`kit_path=${expectedKitPath}`)) {
				await driver.close();
				throw new HarnessInvalid(
					`attached session did not prove scratch SK_PATH ${expectedKitPath} in ${attachedLog}`,
				);
			}
		} else {
			console.error(`[driver] binary: ${BINARY} (explicit probe pin)`);
			driver = await Driver.launch({
				binary: BINARY,
				sandboxHome: true,
				sessionName: `chaos-27-notes-actions-${RUN_ID}`,
				readyTimeoutMs: 30_000,
				defaultTimeoutMs: 10_000,
				env: {
					SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
					SCRIPT_KIT_AGENTIC_KEEP_ACTIONS_WINDOW_OPEN: "1",
				},
			});
		}
	} catch (error) {
		try {
			classifyLaunchError(error);
		} catch (classified) {
			const message =
				classified instanceof Error ? classified.message : String(classified);
			const finding = { phase: "launch", message };
			if (classified instanceof EnvironmentBlocked)
				summary.environmentFindings.push(finding);
			else summary.harnessFindings.push(finding);
			summary.classification =
				classified instanceof EnvironmentBlocked
					? "blocked-by-environment"
					: "invalid-harness";
			summary.pass = false;
			await Bun.write(
				join(OUTPUT_DIR, "summary.json"),
				`${JSON.stringify(summary, null, 2)}\n`,
			);
			console.log(JSON.stringify(summary, null, 2));
			process.exitCode = classified instanceof EnvironmentBlocked ? 3 : 2;
			return;
		}
	}

	const notesDir = ATTACH_HOME
		? join(ATTACH_HOME, ".scriptkit", "brain", "notes")
		: join(driver.sessionDir, "home", ".scriptkit", "brain", "notes");

	async function windows(): Promise<Json[]> {
		const result = (await driver.listAutomationWindows({
			timeoutMs: 5000,
		})) as Json;
		return Array.isArray(result.windows) ? result.windows : [];
	}

	async function notesWindow(): Promise<Json | null> {
		return (
			(await windows()).find(
				(win) =>
					win.id === "notes" ||
					String(win.kind ?? "").toLowerCase() === "notes",
			) ?? null
		);
	}

	async function notesRegistered(): Promise<boolean> {
		return (await notesWindow()) !== null;
	}

	async function notesVisible(): Promise<boolean> {
		const notes = await notesWindow();
		return notes != null && notes.visible !== false;
	}

	async function actionsWindows(): Promise<Json[]> {
		return (await windows()).filter(
			(win) =>
				win.id === "actions-dialog" ||
				win.automationId === "actions-dialog" ||
				String(win.kind ?? "")
					.toLowerCase()
					.includes("action"),
		);
	}

	async function notesState(): Promise<Json> {
		const result = (await driver.request(
			{ type: "getState", target: NOTES_TARGET },
			{ expect: "stateResult", timeoutMs: 6000 },
		)) as Json;
		return (result.notes ?? result) as Json;
	}

	async function dialogState(): Promise<Json | null> {
		try {
			const result = (await driver.request(
				{ type: "getState", target: ACTIONS_TARGET, summaryOnly: true },
				{ expect: "stateResult", timeoutMs: 4000 },
			)) as Json;
			return (result.actionsDialog ?? null) as Json | null;
		} catch {
			const notes = await notesState().catch(() => null);
			const actionState = notes?.commandBars?.actions;
			return actionState?.open === true ? actionState : null;
		}
	}

	async function editorElements(): Promise<Json> {
		return (await driver.getElements(
			{ target: NOTES_TARGET, limit: 260 },
			{ timeoutMs: 6000 },
		)) as Json;
	}

	async function editorValue(): Promise<string> {
		const elements = await editorElements();
		const editor = walk(elements).find(
			(element) =>
				element.semanticId === "input:notes-editor" ||
				element.id === "notes-editor",
		);
		if (typeof editor?.value !== "string") {
			throw new HarnessInvalid(
				"getElements(notes) did not expose input:notes-editor.value",
			);
		}
		return editor.value;
	}

	async function openNotes(): Promise<Json> {
		if (!(await notesVisible())) {
			driver.send({
				type: "openNotes",
				requestId: `${RUN_ID}-open-${Date.now()}`,
			});
		}
		await poll("visible Notes automation target", notesVisible, Boolean, 9000);
		return poll(
			"Notes editor state",
			notesState,
			(state) => Boolean(state.editor && state.view?.surfaceMode === "Notes"),
			9000,
		);
	}

	async function settleNotes(timeoutMs = 5000) {
		return driver.waitForSettle({
			samples: 3,
			intervalMs: 40,
			timeoutMs,
			probe: async () => {
				const state = await notesState();
				return {
					noteId: state.selectedNote?.id ?? null,
					textFingerprint: state.editor?.textFingerprint ?? null,
					selectionRange: state.editor?.selectionRange ?? null,
					dirty: state.dirtyState?.hasUnsavedChanges ?? null,
					focusSurface: state.view?.focusSurface ?? null,
					showActionsPanel: state.view?.showActionsPanel ?? null,
					previewEnabled: state.view?.previewEnabled ?? null,
					actionsOpen: state.commandBars?.actions?.open ?? null,
					noteSwitcherOpen: state.commandBars?.noteSwitcher?.open ?? null,
				};
			},
		});
	}

	async function gpuiKey(
		key: string,
		modifiers: string[] = [],
		target: Json = NOTES_TARGET,
	): Promise<Json> {
		return driver.simulateGpuiEvent(
			{ type: "keyDown", key, modifiers },
			{ target, timeoutMs: 6000 },
		);
	}

	async function setNotesText(text: string, label: string): Promise<Json> {
		const result = (await driver.request(
			{
				type: "batch",
				requestId: `${RUN_ID}-set-${label}-${Date.now()}`,
				target: NOTES_TARGET,
				commands: [{ type: "setInput", text }],
				options: { stopOnError: true, timeout: 6000 },
			},
			{ expect: "batchResult", timeoutMs: 8000 },
		)) as Json;
		if (result.success !== true) {
			throw new HarnessInvalid(
				`setInput(${label}) failed: ${JSON.stringify(result)}`,
			);
		}
		await poll(
			`editor value ${label}`,
			editorValue,
			(value) => value === text,
			6000,
		);
		return result;
	}

	async function selectAll(): Promise<Json> {
		const before = await notesState();
		const expectedBytes = Number(
			before.draftSnapshot?.draft?.bodyByteLength ?? -1,
		);
		const dispatch = await gpuiKey("a", ["cmd"]);
		const after = await poll(
			"Notes select-all",
			notesState,
			(state) =>
				expectedBytes >= 0 &&
				Number(state.editor?.selectionLength ?? -1) === expectedBytes &&
				state.editor?.hasSelection === expectedBytes > 0,
			5000,
		);
		return {
			dispatch,
			expectedBytes,
			selectionRange: after.editor?.selectionRange,
		};
	}

	async function openActions(label: string): Promise<Json> {
		const result = (await driver.request(
			{
				type: "batch",
				requestId: `${RUN_ID}-actions-${label}-${Date.now()}-${Math.random()}`,
				target: NOTES_TARGET,
				commands: [{ type: "openActions" }],
				options: { stopOnError: true, timeout: 6000 },
			},
			{ expect: "batchResult", timeoutMs: 8000 },
		)) as Json;
		if (result.success !== true) {
			throw new HarnessInvalid(
				`openActions(${label}) failed: ${JSON.stringify(result)}`,
			);
		}
		await poll(
			`ActionsDialog open ${label}`,
			async () => ({
				dialog: await dialogState(),
				windows: await actionsWindows(),
			}),
			(value) => Boolean(value.dialog) && value.windows.length === 1,
			7000,
		);
		return result;
	}

	async function dismissActions(label: string): Promise<Json> {
		const dispatch = await gpuiKey("escape", [], NOTES_TARGET);
		await poll(
			`ActionsDialog closed ${label}`,
			async () => ({
				dialog: await dialogState(),
				windows: await actionsWindows(),
			}),
			(value) => !value.dialog && value.windows.length === 0,
			7000,
		);
		return dispatch;
	}

	async function filterActions(text: string, label: string): Promise<Json> {
		const result = (await driver.request(
			{
				type: "batch",
				requestId: `${RUN_ID}-filter-${label}-${Date.now()}`,
				target: ACTIONS_TARGET,
				commands: [{ type: "setInput", text }],
				options: { stopOnError: true, timeout: 5000 },
			},
			{ expect: "batchResult", timeoutMs: 7000 },
		)) as Json;
		if (result.success !== true) {
			throw new HarnessInvalid(
				`filterActions(${label}) failed: ${JSON.stringify(result)}`,
			);
		}
		await poll(
			`ActionsDialog filter ${label}`,
			dialogState,
			(dialog) => Number(dialog?.search?.textLength ?? -1) === [...text].length,
			5000,
		);
		return result;
	}

	async function selectAction(id: string): Promise<Json> {
		let semanticId: string | null = null;
		const dialog = await dialogState();
		const row = visibleActions(dialog).find(
			(candidate) => actionId(candidate) === id,
		);
		if (typeof row?.semanticId === "string") semanticId = row.semanticId;
		if (!semanticId) {
			const elements = (await driver.getElements(
				{ target: ACTIONS_TARGET, limit: 260 },
				{ timeoutMs: 5000 },
			)) as Json;
			const node = walk(elements).find((candidate) =>
				String(candidate.semanticId ?? "").endsWith(`:${id}`),
			);
			semanticId =
				typeof node?.semanticId === "string" ? node.semanticId : null;
		}
		if (!semanticId) {
			throw new HarnessInvalid(
				`ActionsDialog did not expose a semantic row for ${id}`,
			);
		}
		const result = (await driver.request(
			{
				type: "batch",
				requestId: `${RUN_ID}-select-${id}-${Date.now()}`,
				target: ACTIONS_TARGET,
				commands: [{ type: "selectBySemanticId", semanticId }],
				options: { stopOnError: true, timeout: 5000 },
			},
			{ expect: "batchResult", timeoutMs: 7000 },
		)) as Json;
		if (result.success !== true) {
			throw new HarnessInvalid(
				`select action ${id} failed: ${JSON.stringify(result)}`,
			);
		}
		await poll(
			`selected action ${id}`,
			dialogState,
			(state) => selectedActionId(state) === id,
			5000,
		);
		return { semanticId, result };
	}

	async function activateAction(id: string, title: string): Promise<Json> {
		await openActions(`activate-${id}`);
		await filterActions(title, `activate-${id}`);
		const selected = await selectAction(id);
		const before = await dialogState();
		const dispatch = await gpuiKey("enter", [], ACTIONS_TARGET);
		await poll(
			`action ${id} closed dialog`,
			async () => ({
				dialog: await dialogState(),
				windows: await actionsWindows(),
			}),
			(value) => !value.dialog && value.windows.length === 0,
			7000,
		);
		return { selected, before, dispatch };
	}

	function noteFiles(): string[] {
		if (!existsSync(notesDir)) return [];
		return readdirSync(notesDir)
			.filter((name) => name.endsWith(".md") && !name.includes(".conflict-"))
			.map((name) => join(notesDir, name));
	}

	function fileContaining(marker: string): string | null {
		for (const path of noteFiles()) {
			try {
				if (readFileSync(path, "utf8").includes(marker)) return path;
			} catch {
				// Atomic replacement can race directory enumeration; the poll retries.
			}
		}
		return null;
	}

	async function waitForCanonical(
		marker: string,
		expected: string,
	): Promise<Json> {
		return poll(
			`canonical note containing ${marker}`,
			async () => {
				const path = fileContaining(marker);
				const content = path ? readFileSync(path, "utf8") : "";
				const state = await notesState();
				return { path, content, state };
			},
			(value) =>
				Boolean(value.path) &&
				value.content.includes(expected) &&
				value.state.dirtyState?.hasUnsavedChanges === false &&
				value.state.selectedNote?.contentFingerprint === fnv1a64(expected) &&
				value.state.editor?.textFingerprint === fnv1a64(expected),
			9000,
		);
	}

	async function diagnosticCall(
		label: string,
		call: () => Promise<unknown>,
	): Promise<Json> {
		try {
			return (await call()) as Json;
		} catch (error) {
			return {
				label,
				error: error instanceof Error ? error.message : String(error),
			};
		}
	}

	async function baselineErrors(): Promise<Set<string>> {
		const result = (await driver
			.getLogs({ level: "error", limit: 500 }, { timeoutMs: 5000 })
			.catch(() => ({ entries: [] }))) as Json;
		const entries = (result.entries ?? result.logs ?? []) as Json[];
		return new Set(
			entries.map((entry) => `${entry.target ?? ""}|${entry.message ?? ""}`),
		);
	}

	async function errorDelta(before: Set<string>): Promise<string[]> {
		const after = await baselineErrors();
		return [...after].filter((entry) => !before.has(entry));
	}

	async function closeNotesForCleanup(): Promise<Json> {
		if (await dialogState()) await dismissActions("cleanup");
		const before = await notesWindow();
		let toggleError: string | null = null;
		if (before && before.visible !== false) {
			// `openNotes` is the documented toggle. Cleanup must not rely on a
			// focus-sensitive Escape delivery path; that behavior is judged only by
			// the dedicated escape-ladder row once the SCREEN queue is available.
			driver.send({
				type: "openNotes",
				requestId: `${RUN_ID}-cleanup-toggle-${Date.now()}`,
			});
			try {
				await poll(
					"Notes toggle closed or hid the window",
					notesVisible,
					(visible) => !visible,
					8000,
				);
			} catch (error) {
				toggleError = error instanceof Error ? error.message : String(error);
			}
		}
		driver.send({ type: "hide" });
		const main = await poll(
			"main window hidden after Notes cleanup",
			async () => (await driver.getState({ timeoutMs: 6000 })) as Json,
			(state) => state.windowVisible === false,
			8000,
		);
		return {
			before,
			after: await notesWindow(),
			mainWindowVisible: main.windowVisible ?? null,
			toggleError,
		};
	}

	async function executeRow(
		rowId: RowId,
		body: (
			receipt: RowReceipt,
			check: (
				name: string,
				ok: boolean,
				kind?: CheckKind,
				detail?: Json,
			) => void,
		) => Promise<void>,
	): Promise<RowReceipt> {
		const startedAtMs = Date.now();
		const rowDir = join(OUTPUT_DIR, rowId);
		mkdirSync(rowDir, { recursive: true });
		const receipt: RowReceipt = {
			schemaVersion: 1,
			rowId,
			classification: "running",
			checks: [],
			productFindings: [],
			harnessFindings: [],
			environmentFindings: [],
			evidence: {},
			startedAt: new Date(startedAtMs).toISOString(),
		};
		const stateSamples: Json[] = [];
		const windowSamples: Json[] = [];
		const captureSample = async (label: string) => {
			const capturedAt = new Date().toISOString();
			const [main, notes, windowTable] = await Promise.all([
				diagnosticCall(`${label}:main-state`, () =>
					driver.getState({ timeoutMs: 6000 }),
				),
				diagnosticCall(`${label}:notes-state`, notesState),
				diagnosticCall(`${label}:windows`, () =>
					driver.listAutomationWindows({ timeoutMs: 6000 }),
				),
			]);
			stateSamples.push({ label, capturedAt, main, notes });
			windowSamples.push({ label, capturedAt, windowTable });
		};
		const beforeErrors = await baselineErrors();
		const check = (
			name: string,
			ok: boolean,
			kind: CheckKind = "product",
			detail: Json = {},
		) => {
			const entry = { name, ok, kind, detail };
			receipt.checks.push(entry);
			if (!ok) {
				const finding = { name, detail };
				if (kind === "product") receipt.productFindings.push(finding);
				else if (kind === "environment")
					receipt.environmentFindings.push(finding);
				else receipt.harnessFindings.push(finding);
			}
		};
		try {
			await openNotes();
			await captureSample("before-row");
			await body(receipt, check);
			const freshErrors = await errorDelta(beforeErrors);
			check("no_new_error_logs", freshErrors.length === 0, "product", {
				freshErrors,
			});
		} catch (error) {
			const message = error instanceof Error ? error.message : String(error);
			if (error instanceof EnvironmentBlocked)
				receipt.environmentFindings.push({ message });
			else if (error instanceof HarnessInvalid)
				receipt.harnessFindings.push({ message });
			else
				receipt.harnessFindings.push({ message, unclassifiedException: true });
		}

		await captureSample("after-row");
		const capturedAt = new Date().toISOString();
		const [layout, elements, logs] = await Promise.all([
			diagnosticCall("layout", async () => ({
				notes: await diagnosticCall("layout:notes", () =>
					driver.getLayoutInfo({ target: NOTES_TARGET }, { timeoutMs: 6000 }),
				),
				main: await diagnosticCall("layout:main", () =>
					driver.getLayoutInfo(
						{ target: { type: "main" } },
						{ timeoutMs: 6000 },
					),
				),
			})),
			diagnosticCall("elements", async () => ({
				notes: await diagnosticCall("elements:notes", () =>
					driver.getElements(
						{ target: NOTES_TARGET, limit: 300 },
						{ timeoutMs: 6000 },
					),
				),
				main: await diagnosticCall("elements:main", () =>
					driver.getElements(
						{ target: { type: "main" }, limit: 300 },
						{ timeoutMs: 6000 },
					),
				),
			})),
			diagnosticCall("logs", () =>
				driver.getLogs({ limit: 500 }, { timeoutMs: 6000 }),
			),
		]);
		const finishedAtMs = Date.now();
		const timings = {
			rowId,
			startedAtMs,
			finishedAtMs,
			durationMs: finishedAtMs - startedAtMs,
			capturedAt,
		};
		await Promise.all([
			writeJson(join(rowDir, "layout.json"), layout),
			writeJson(join(rowDir, "elements.json"), elements),
			writeJson(join(rowDir, "app-logs.json"), logs),
			writeJson(join(rowDir, "state-samples.json"), stateSamples),
			writeJson(join(rowDir, "windows.json"), windowSamples),
			writeJson(join(rowDir, "timings.json"), timings),
		]);
		receipt.evidence = {
			...receipt.evidence,
			diagnosticBundle: {
				rowDir,
				layout: "layout.json",
				elements: "elements.json",
				logs: "app-logs.json",
				stateSamples: "state-samples.json",
				windows: "windows.json",
				timings: "timings.json",
			},
		};
		receipt.classification = receipt.environmentFindings.length
			? "blocked-by-environment"
			: receipt.harnessFindings.length
				? "invalid-harness"
				: receipt.productFindings.length
					? "failed-product"
					: "verified";
		receipt.finishedAt = new Date(finishedAtMs).toISOString();
		await writeReceipt(receipt);
		summary.rowReceipts.push({ rowId, classification: receipt.classification });
		summary.productFindings.push(
			...receipt.productFindings.map((finding) => ({ rowId, ...finding })),
		);
		summary.harnessFindings.push(
			...receipt.harnessFindings.map((finding) => ({ rowId, ...finding })),
		);
		summary.environmentFindings.push(
			...receipt.environmentFindings.map((finding) => ({ rowId, ...finding })),
		);
		return receipt;
	}

	const rowBodies: Record<
		RowId,
		(
			receipt: RowReceipt,
			check: (
				name: string,
				ok: boolean,
				kind?: CheckKind,
				detail?: Json,
			) => void,
		) => Promise<void>
	> = {
		"action-menu-races": async (receipt, check) => {
			const issueOpen = (label: string) =>
				driver.request(
					{
						type: "batch",
						requestId: `${RUN_ID}-actions-race-${label}-${Date.now()}-${Math.random()}`,
						target: NOTES_TARGET,
						commands: [{ type: "openActions" }],
						options: { stopOnError: true, timeout: 6000 },
					},
					{ expect: "batchResult", timeoutMs: 8000 },
				);
			const [first, second] = await Promise.allSettled([
				issueOpen("open-while-opening-a"),
				issueOpen("open-while-opening-b"),
			]);
			await poll(
				"open-while-opening settles to one ActionsDialog",
				async () => ({
					dialog: await dialogState(),
					windows: await actionsWindows(),
				}),
				(value) => Boolean(value.dialog) && value.windows.length === 1,
				7000,
			);
			const dialog = await dialogState();
			const actionRows = visibleActions(dialog);
			const selected = selectedActionId(dialog);
			const actionWindowCount = (await actionsWindows()).length;
			check(
				"open_while_opening_keeps_one_popup",
				actionWindowCount === 1,
				"product",
				{
					actionWindowCount,
					firstOutcome:
						first.status === "fulfilled"
							? (first.value.success ?? null)
							: String(first.reason),
					secondOutcome:
						second.status === "fulfilled"
							? (second.value.success ?? null)
							: String(second.reason),
				},
			);
			check(
				"open_while_opening_selection_coherent",
				Boolean(selected) &&
					actionRows.some((row) => actionId(row) === selected),
				"product",
				{ selected, visibleActionIds: actionRows.map(actionId) },
			);
			await dismissActions("open-while-opening");

			const cycles: Json[] = [];
			for (let i = 0; i < 12; i += 1) {
				const open = await driver.request(
					{
						type: "batch",
						requestId: `${RUN_ID}-rapid-open-${i}`,
						target: NOTES_TARGET,
						commands: [{ type: "openActions" }],
						options: { stopOnError: true, timeout: 3000 },
					},
					{ expect: "batchResult", timeoutMs: 5000 },
				);
				const escape = await gpuiKey("escape", [], NOTES_TARGET);
				cycles.push({
					i,
					openSuccess: open.success ?? null,
					escapeSuccess: escape.success ?? null,
				});
			}
			const settle = await settleNotes(7000);
			const after = {
				notesRegistered: await notesRegistered(),
				dialog: await dialogState(),
				actionWindowCount: (await actionsWindows()).length,
			};
			check(
				"rapid_open_dismiss_keeps_notes_alive",
				after.notesRegistered,
				"product",
				after,
			);
			check(
				"rapid_open_dismiss_leaves_no_popup",
				!after.dialog && after.actionWindowCount === 0,
				"product",
				after,
			);
			receipt.evidence = { cycles, settle, after };
		},

		"hostile-formatting": async (receipt, check) => {
			if ((await notesState()).view?.showFormatToolbar !== true) {
				const before = (await notesState()).view?.showFormatToolbar;
				const activation = await activateAction("format", "Format");
				const after = await poll(
					"format toolbar open",
					notesState,
					(state) => state.view?.showFormatToolbar === true,
					6000,
				);
				receipt.evidence.formatMenuActivation = {
					before,
					after: after.view?.showFormatToolbar,
					activation,
				};
			}

			const ZALGO = "z̸̢̛̭̙̘̋̽͐̕a̶͖͇͐̈́l̷̜̈́̊g̶̻̈o̸̙͌";
			const RTL = "שלום עולם مرحبا بالعالم mixed direction";
			const ZWJ = "family 👨‍👩‍👧‍👦 flag 🏳️‍🌈 a​b‌c‍d";
			const cases = [
				{
					id: "zalgo",
					seed: `${ZALGO} ${RUN_ID}`,
					key: "b",
					mods: ["cmd"],
					wrap: ["**", "**"],
				},
				{
					id: "rtl",
					seed: `${RTL} ${RUN_ID}`,
					key: "i",
					mods: ["cmd"],
					wrap: ["_", "_"],
				},
				{
					id: "zwj",
					seed: `${ZWJ} ${RUN_ID}`,
					key: "e",
					mods: ["cmd"],
					wrap: ["`", "`"],
				},
				{
					id: "huge",
					// Keep the JSONL command below the documented 16 KiB stdin cap while
					// still exercising a multi-KiB hostile UTF-8 selection.
					seed: `huge-${RUN_ID}-` + `${ZALGO}|${RTL}|${ZWJ}|`.repeat(55),
					key: "b",
					mods: ["cmd"],
					wrap: ["**", "**"],
				},
			];
			const casesEvidence: Json[] = [];
			for (const spec of cases) {
				await setNotesText(spec.seed, `format-${spec.id}`);
				const selection = await selectAll();
				const expected = `${spec.wrap[0]}${spec.seed}${spec.wrap[1]}`;
				const dispatch = await gpuiKey(spec.key, spec.mods, NOTES_TARGET);
				const after = await poll(
					`format ${spec.id}`,
					editorValue,
					(value) => value === expected,
					7000,
				);
				const canonical = await waitForCanonical(
					`huge-${RUN_ID}` === spec.seed.slice(0, 5 + RUN_ID.length)
						? `huge-${RUN_ID}`
						: RUN_ID,
					expected,
				);
				const noReplacement = !canonical.content.includes("�");
				check(
					`format_${spec.id}_memory_roundtrip`,
					after === expected,
					"product",
					{
						expectedBytes: new TextEncoder().encode(expected).length,
						actualBytes: new TextEncoder().encode(after).length,
					},
				);
				check(
					`format_${spec.id}_autosave_roundtrip`,
					Boolean(canonical.path),
					"product",
					{
						path: canonical.path,
						diskBytes: new TextEncoder().encode(canonical.content).length,
					},
				);
				check(`format_${spec.id}_utf8_intact`, noReplacement, "product", {
					replacementCharacterFound: !noReplacement,
				});
				casesEvidence.push({
					id: spec.id,
					selection,
					dispatch,
					canonicalPath: canonical.path,
				});
			}

			const multibyteSeed = "A🙂B";
			await setNotesText(multibyteSeed, "format-multibyte-boundary");
			await gpuiKey("left", [], NOTES_TARGET);
			await gpuiKey("left", ["shift"], NOTES_TARGET);
			const selected = await poll(
				"multibyte scalar selection",
				notesState,
				(state) => Number(state.editor?.selectionLength ?? -1) === 4,
				5000,
			);
			const range = selected.editor?.selectionRange as number[];
			const expectedMultibyte = "A**🙂**B";
			await gpuiKey("b", ["cmd"], NOTES_TARGET);
			const afterMultibyte = await poll(
				"multibyte formatting",
				editorValue,
				(value) => value === expectedMultibyte,
				6000,
			);
			const canonicalMultibyte = await waitForCanonical(
				expectedMultibyte,
				expectedMultibyte,
			);
			check(
				"multibyte_selection_uses_utf8_boundaries",
				range?.[1] - range?.[0] === 4 && afterMultibyte === expectedMultibyte,
				"product",
				{ range, expectedByteWidth: 4, canonicalPath: canonicalMultibyte.path },
			);
			receipt.evidence.cases = casesEvidence;
		},

		"enter-semantics": async (receipt, check) => {
			const beforePreview = Boolean((await notesState()).view?.previewEnabled);
			const actionable = await activateAction(
				"toggle_preview",
				"Toggle Preview",
			);
			const afterPreview = Boolean((await notesState()).view?.previewEnabled);
			check(
				"enter_action_row_executes_once",
				afterPreview !== beforePreview,
				"product",
				{
					beforePreview,
					afterPreview,
					selectedBefore: selectedActionId(actionable.before),
				},
			);

			await openActions("enter-zero-match");
			const beforeEditor = await notesState();
			const hostileFilter = `no-match-‮-${RUN_ID}`;
			await filterActions(hostileFilter, "enter-zero-match");
			const zeroDialog = await dialogState();
			const zeroRows = visibleActions(zeroDialog);
			const enter = await gpuiKey("enter", [], ACTIONS_TARGET);
			const afterEnter = await settleNotes(3000);
			const afterDialog = await dialogState();
			const afterEditor = await notesState();
			check(
				"enter_zero_match_executes_nothing",
				zeroRows.length === 0,
				"product",
				{
					zeroRows: zeroRows.map(actionId),
					selectedActionId: selectedActionId(zeroDialog),
				},
			);
			check(
				"enter_zero_match_preserves_editor_and_popup",
				afterEditor.editor?.textFingerprint ===
					beforeEditor.editor?.textFingerprint && Boolean(afterDialog),
				"product",
				{
					beforeFingerprint: beforeEditor.editor?.textFingerprint,
					afterFingerprint: afterEditor.editor?.textFingerprint,
					popupOpen: Boolean(afterDialog),
				},
			);
			await dismissActions("enter-zero-match");

			await openActions("enter-structural-rows");
			const structural = await dialogState();
			const geometryRows = Array.isArray(structural?.rowGeometry?.rows)
				? structural.rowGeometry.rows
				: [];
			const structuralRows = geometryRows.filter(
				(row: Json) => row.kind !== "action",
			);
			const selectedRow = structural?.rowGeometry?.selectedRow;
			check(
				"structural_rows_are_not_enter_targets",
				structuralRows.length > 0 && selectedRow?.kind === "action",
				structuralRows.length === 0 ? "harness" : "product",
				{
					structuralKinds: [
						...new Set(structuralRows.map((row: Json) => row.kind)),
					],
					selectedRowKind: selectedRow?.kind ?? null,
					missingPrimitive:
						structuralRows.length === 0
							? "ActionsDialog rowGeometry structural rows"
							: null,
				},
			);
			await dismissActions("enter-structural-rows");
			let previewRestore: Json | null = null;
			if (afterPreview !== beforePreview) {
				previewRestore = await activateAction(
					"toggle_preview",
					"Toggle Preview",
				);
				await poll(
					"restore preview state after Enter semantics row",
					notesState,
					(state) => Boolean(state.view?.previewEnabled) === beforePreview,
					6000,
				);
			}
			receipt.evidence = {
				actionable,
				zeroMatch: { enter, settle: afterEnter },
				previewRestore,
			};
		},

		"escape-ladder": async (receipt, check) => {
			await openActions("escape-ladder");
			const first = await dismissActions("escape-ladder-first");
			const afterFirst = await notesState();
			const elements = await editorElements();
			const focused = elements.focusedSemanticId ?? null;
			check(
				"first_escape_dismisses_popup_and_keeps_notes",
				!afterFirst.view?.showActionsPanel &&
					(await actionsWindows()).length === 0,
				"product",
				{ showActionsPanel: afterFirst.view?.showActionsPanel, focused },
			);
			check(
				"first_escape_restores_editor_focus",
				focused === "input:notes-editor",
				"product",
				{
					focusedSemanticId: focused,
				},
			);
			const second = await gpuiKey("escape", [], NOTES_TARGET);
			let closeObservationError: string | null = null;
			try {
				await poll(
					"second Escape closes Notes",
					notesRegistered,
					(registered) => !registered,
					8000,
				);
			} catch (error) {
				closeObservationError =
					error instanceof Error ? error.message : String(error);
			}
			const list = await windows();
			const main = (await driver.getState({ timeoutMs: 6000 })) as Json;
			const closedHonestly =
				!list.some((win) => win.id === "notes") &&
				!list.some((win) => win.id === "actions-dialog");
			check("second_escape_closes_notes_honestly", closedHonestly, "product", {
				second,
				windows: list.map((win) => ({
					id: win.id,
					kind: win.kind,
					focused: win.focused,
					visible: win.visible,
				})),
				mainWindowVisible: main.windowVisible ?? null,
				closeObservationError,
			});
			receipt.evidence = {
				first,
				second,
				closeObservationError,
				mainWindowVisibleAfterNotesClose: main.windowVisible ?? null,
			};
		},

		"autosave-race": async (receipt, check) => {
			const baseline = `autosave-baseline-${RUN_ID}`;
			await setNotesText(baseline, "autosave-baseline");
			const baselineDisk = await waitForCanonical(RUN_ID, baseline);
			const canonicalPath = String(baselineDisk.path);
			const pending = `autosave-pending-${RUN_ID}-${"é🙂".repeat(120)}`;
			const expected = `**${pending}**`;
			const samples: Json[] = [];
			await setNotesText(pending, "autosave-pending");

			let monitorDone = false;
			const monitor = (async () => {
				const started = performance.now();
				while (performance.now() - started < 3500 && !monitorDone) {
					try {
						const content = readFileSync(canonicalPath, "utf8");
						const state = content.includes(expected)
							? "formatted"
							: content.includes(pending)
								? "pending"
								: content.includes(baseline)
									? "baseline"
									: "other";
						samples.push({
							elapsedMs: Math.round(performance.now() - started),
							bytes: new TextEncoder().encode(content).length,
							state,
							replacementCharacter: content.includes("�"),
						});
					} catch (error) {
						samples.push({
							elapsedMs: Math.round(performance.now() - started),
							state: "read-error",
							error: String(error),
						});
					}
					await Bun.sleep(10);
				}
			})();

			const selection = await selectAll();
			const actionDispatch = await gpuiKey("b", ["cmd"], NOTES_TARGET);
			const final = await waitForCanonical(RUN_ID, expected);
			monitorDone = true;
			await monitor;
			const forbidden = samples.filter(
				(sample) =>
					sample.state === "other" ||
					sample.state === "read-error" ||
					sample.bytes === 0 ||
					sample.replacementCharacter === true,
			);
			check(
				"autosave_action_race_final_editor_and_disk_match",
				final.content.includes(expected),
				"product",
				{
					canonicalPath,
					finalFingerprint: final.state.editor?.textFingerprint,
					expectedFingerprint: fnv1a64(expected),
				},
			);
			check(
				"autosave_action_race_has_no_torn_observation",
				forbidden.length === 0,
				"product",
				{
					sampleCount: samples.length,
					forbidden: forbidden.slice(0, 12),
					states: [...new Set(samples.map((sample) => sample.state))],
				},
			);
			receipt.evidence = { canonicalPath, selection, actionDispatch, samples };
		},

		"popup-key-storm": async (receipt, check) => {
			const beforePreview = Boolean((await notesState()).view?.previewEnabled);
			await openActions("key-storm");
			await filterActions("Toggle Preview", "key-storm");
			await selectAction("toggle_preview");
			const baseline = await dialogState();
			const baselineSelected = selectedActionId(baseline);
			const keys = ["down", "up", "home", "end", "pageup", "pagedown"];
			const samples: Json[] = [];
			for (let i = 0; i < 48; i += 1) {
				const key = keys[i % keys.length];
				const dispatch = await gpuiKey(key, [], ACTIONS_TARGET);
				const state = await dialogState();
				const visible = visibleActions(state).map(actionId);
				samples.push({
					i,
					key,
					dispatchSuccess: dispatch.success ?? null,
					selected: selectedActionId(state),
					visible,
				});
			}
			const unstable = samples.filter(
				(sample) =>
					sample.selected !== baselineSelected ||
					!sample.visible.includes(sample.selected),
			);
			check(
				"popup_key_storm_selection_stable",
				unstable.length === 0,
				"product",
				{
					baselineSelected,
					unstable: unstable.slice(0, 10),
					sampleCount: samples.length,
				},
			);
			const enter = await gpuiKey("enter", [], ACTIONS_TARGET);
			await poll(
				"storm-selected action closes popup",
				async () => ({
					dialog: await dialogState(),
					windows: await actionsWindows(),
				}),
				(value) => !value.dialog && value.windows.length === 0,
				7000,
			);
			const afterPreview = Boolean((await notesState()).view?.previewEnabled);
			check(
				"popup_key_storm_executes_selected_action_once",
				afterPreview !== beforePreview,
				"product",
				{
					beforePreview,
					afterPreview,
					baselineSelected,
				},
			);
			receipt.evidence = { samples, enter };
		},
	};

	try {
		for (const row of rows) {
			await executeRow(row, rowBodies[row]);
			// Rows are incremental but share one scratch HOME. Re-establish Notes
			// only through its real open path; do not reset product state in-place.
			if (row !== rows.at(-1) && !(await notesRegistered())) await openNotes();
		}

		let cleanup: Json;
		try {
			cleanup = await closeNotesForCleanup();
			const main = (await driver.getState({ timeoutMs: 6000 })) as Json;
			cleanup.mainWindowVisible = main.windowVisible ?? null;
			cleanup.notesRegistered = await notesRegistered();
			cleanup.actionsWindowCount = (await actionsWindows()).length;
			if (
				cleanup.notesRegistered ||
				cleanup.actionsWindowCount !== 0 ||
				cleanup.mainWindowVisible === true
			) {
				summary.productFindings.push({
					rowId: "cleanup",
					name: "cleanup_gate_failed",
					detail: cleanup,
				});
			}
		} catch (error) {
			cleanup = {
				error: error instanceof Error ? error.message : String(error),
			};
			summary.harnessFindings.push({
				rowId: "cleanup",
				name: "cleanup_unobservable",
				detail: cleanup,
			});
		}
		summary.cleanup = cleanup;
	} finally {
		summary.sessionDir = driver.sessionDir;
		summary.appLog =
			driver instanceof Driver
				? driver.logPath
				: join(driver.sessionDir, "app.log");
		summary.externalSessionLeftRunning = driver instanceof AttachedDriver;
		await driver.close();
		const processPattern = `^${BINARY.replace(/[\\^$.*+?()[\]{}|]/g, "\\$&")}([[:space:]]|$)`;
		const processProbe = Bun.spawnSync(["pgrep", "-f", processPattern], {
			stdout: "pipe",
			stderr: "pipe",
		});
		const stdout = new TextDecoder().decode(processProbe.stdout).trim();
		const stderr = new TextDecoder().decode(processProbe.stderr).trim();
		const processCheck = {
			command: ["pgrep", "-f", processPattern],
			exitCode: processProbe.exitCode,
			stdout,
			stderr,
			clean: processProbe.exitCode === 1 && stdout.length === 0,
			checkedAt: new Date().toISOString(),
		};
		summary.postTeardownProcessCheck = processCheck;
		await writeJson(
			join(OUTPUT_DIR, "post-teardown-process.json"),
			processCheck,
		);
		if (!processCheck.clean) {
			summary.harnessFindings.push({
				rowId: "postTeardown",
				name: "binary_process_remained_after_driver_close",
				detail: processCheck,
			});
		}
	}

	summary.classification = summary.environmentFindings.length
		? "blocked-by-environment"
		: summary.harnessFindings.length
			? "invalid-harness"
			: summary.productFindings.length
				? "failed-product"
				: "verified";
	summary.pass = summary.classification === "verified";
	await Bun.write(
		join(OUTPUT_DIR, "summary.json"),
		`${JSON.stringify(summary, null, 2)}\n`,
	);
	console.log(JSON.stringify(summary, null, 2));
	process.exitCode = summary.pass
		? 0
		: summary.classification === "blocked-by-environment"
			? 3
			: summary.classification === "invalid-harness"
				? 2
				: 1;
}

await runBattery();

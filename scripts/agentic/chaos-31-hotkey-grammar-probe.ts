#!/usr/bin/env bun
/**
 * NN=31 hotkey gesture grammar battery.
 *
 * One sandboxed launch sweeps selected rows and writes a complete rule-12
 * diagnostic bundle per row. Timing-boundary classifier cells stay in Rust
 * unit tests; this probe covers observable surface routing with coarse margins.
 *
 * Usage (only after a named SCREEN release):
 *   PROBE_BINARY=target-agent/artifacts/finder-hotkeys/script-kit-gpui \
 *   PROBE_OUTPUT_DIR=.test-output/chaos-31-hotkey-grammar/<lane>-<run> \
 *   PROBE_SCREEN_CLAIMED=1 \
 *   bun scripts/agentic/chaos-31-hotkey-grammar-probe.ts [--rows a,b]
 */
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const HOLD_MS = 250;
const DOUBLE_MS = 300;
const BINARY =
	process.env.PROBE_BINARY ??
	"target-agent/artifacts/finder-hotkeys/script-kit-gpui";
const RUN_ID =
	process.env.NN31_RUN_ID ??
	`finder-hotkeys-${Date.now().toString(36)}-${process.pid}`;
const OUTPUT_DIR = resolve(
	process.env.PROBE_OUTPUT_DIR ??
		`.test-output/chaos-31-hotkey-grammar/${RUN_ID}`,
);
const SESSION_NAME = process.env.NN31_SESSION ?? `finder-hotkeys-${RUN_ID}`;

const ALL_ROWS = [
	"tap-closed-instant",
	"tap-open-empty-immediate",
	"tap-open-filtered-deferred",
	"hold-closed-day-page",
	"hold-open-inert",
	"tap-open-day-page-immediate",
	"double-tap-closed-clean-chat",
	"double-tap-open-after-preview",
	"key-repeat-storm-single-hold",
	"space-first-day-page",
	"space-prefixed-query-control",
	"semicolon-first-capture-picker",
	"semicolon-target-accept",
	"semicolon-ordinary-text-control",
	"escape-hide-resync",
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
	if (unknown.length > 0)
		throw new HarnessInvalid(`unknown rows: ${unknown.join(", ")}`);
	if (requested.length === 0)
		throw new HarnessInvalid("--rows selected no rows");
	return requested as RowId[];
}

async function writeJson(path: string, value: unknown): Promise<void> {
	await Bun.write(path, `${JSON.stringify(value, null, 2)}\n`);
}

function classifyLaunchError(
	error: unknown,
): EnvironmentBlocked | HarnessInvalid {
	const message = error instanceof Error ? error.message : String(error);
	if (
		/did not become ready|rpc.*timeout|timed out|operation not permitted|sandbox/i.test(
			message,
		)
	) {
		return new EnvironmentBlocked(message);
	}
	return new HarnessInvalid(message);
}

async function diagnosticCall<T>(
	label: string,
	call: () => Promise<T>,
): Promise<T | Json> {
	try {
		return await call();
	} catch (error) {
		return {
			diagnosticError: label,
			message: error instanceof Error ? error.message : String(error),
		};
	}
}

function logEntries(payload: Json): Json[] {
	return Array.isArray(payload?.entries) ? payload.entries : [];
}

function logFingerprint(entry: Json): string {
	return JSON.stringify(entry);
}

function collectLabels(node: unknown, labels: string[] = []): string[] {
	if (!node || typeof node !== "object") return labels;
	if (Array.isArray(node)) {
		for (const child of node) collectLabels(child, labels);
		return labels;
	}
	const json = node as Json;
	for (const key of ["semanticId", "id", "text", "value", "label", "title"]) {
		if (typeof json[key] === "string") labels.push(json[key]);
	}
	for (const value of Object.values(json)) collectLabels(value, labels);
	return labels;
}

async function runBattery(): Promise<void> {
	const rows = selectedRows();
	const summary: Json = {
		schemaVersion: 1,
		tool: "chaos-31-hotkey-grammar-probe",
		nn: 31,
		runId: RUN_ID,
		binary: resolve(BINARY),
		outputDir: OUTPUT_DIR,
		holdMs: HOLD_MS,
		doubleMs: DOUBLE_MS,
		rows,
		rowReceipts: [],
		productFindings: [],
		harnessFindings: [],
		environmentFindings: [],
	};
	if (process.env.PROBE_SCREEN_CLAIMED !== "1") {
		summary.classification = "invalid-harness";
		summary.harnessFindings.push({
			name: "screen_claim_missing",
			message:
				"Set PROBE_SCREEN_CLAIMED=1 only after the ledger release names this runner.",
		});
		await writeJson(join(OUTPUT_DIR, "summary.json"), summary);
		console.error(JSON.stringify(summary, null, 2));
		process.exit(2);
	}
	let driver: Driver;
	try {
		console.error(`[driver] binary: ${resolve(BINARY)} (explicit NN=31 pin)`);
		driver = await Driver.launch({
			binary: BINARY,
			sandboxHome: true,
			sessionName: SESSION_NAME,
			readyTimeoutMs: 15_000,
			defaultTimeoutMs: 8_000,
			env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
		});
	} catch (error) {
		const classified = classifyLaunchError(error);
		summary.classification =
			classified instanceof EnvironmentBlocked
				? "blocked-by-environment"
				: "invalid-harness";
		summary[
			classified instanceof EnvironmentBlocked
				? "environmentFindings"
				: "harnessFindings"
		].push({
			name: "launch_failed",
			message: classified.message,
		});
		await writeJson(join(OUTPUT_DIR, "summary.json"), summary);
		console.log(JSON.stringify(summary, null, 2));
		process.exit(summary.classification === "blocked-by-environment" ? 3 : 2);
	}

	const gesture = (phase: "down" | "up", label: string) =>
		driver.request(
			{
				type: "simulateMainHotkeyGesture",
				phase,
				requestId: `${RUN_ID}-${label}`,
			},
			{ expect: "externalCommandResult", timeoutMs: 5_000 },
		);
	const state = () => driver.getState({ timeoutMs: 6_000 }) as Promise<Json>;
	const agentChatState = async (): Promise<Json> => {
		const result = (await driver.request(
			{ type: "getAgentChatState" },
			{ timeoutMs: 8_000 },
		)) as Json;
		return (result.state ?? result) as Json;
	};
	const windows = () =>
		driver.listAutomationWindows({ timeoutMs: 6_000 }) as Promise<Json>;
	const sampleMainWindow = async (): Promise<Json | null> => {
		const table = await windows();
		const rows = Array.isArray(table.windows) ? table.windows : [];
		return rows.find((row: Json) => row.id === "main") ?? null;
	};
	const hide = async (label: string) => {
		driver.send({ type: "hide", requestId: `${RUN_ID}-${label}` });
		await driver.waitForState({ windowVisible: false }, { timeoutMs: 8_000 });
		await Bun.sleep(40);
	};
	const ensureHidden = async (label: string) => {
		const before = await state();
		if (before.windowVisible === true) await hide(label);
	};
	const openingTap = async (label: string) => {
		await ensureHidden(`${label}-prehide`);
		await gesture("down", `${label}-down`);
		await gesture("up", `${label}-up`);
		await driver.waitForState({ windowVisible: true }, { timeoutMs: 8_000 });
		await Bun.sleep(DOUBLE_MS + 60);
	};

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
			sample: (label: string, detail?: Json) => Promise<void>,
		) => Promise<void>,
	): Promise<void> {
		const rowDir = join(OUTPUT_DIR, rowId);
		mkdirSync(rowDir, { recursive: true });
		const startedAtMs = Date.now();
		const stateSamples: Json[] = [];
		const windowSamples: Json[] = [];
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
		const beforeLogs = (await driver.getLogs(
			{ limit: 2_000 },
			{ timeoutMs: 6_000 },
		)) as Json;
		const baseline = new Set(logEntries(beforeLogs).map(logFingerprint));
		const sample = async (label: string, detail: Json = {}) => {
			const capturedAt = new Date().toISOString();
			const [main, windowTable] = await Promise.all([
				diagnosticCall(`${label}:state`, state),
				diagnosticCall(`${label}:windows`, windows),
			]);
			stateSamples.push({ label, capturedAt, main, detail });
			windowSamples.push({ label, capturedAt, windowTable });
		};
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
			await sample("before-row");
			await body(receipt, check, sample);
		} catch (error) {
			const message =
				error instanceof Error ? (error.stack ?? error.message) : String(error);
			if (error instanceof EnvironmentBlocked)
				receipt.environmentFindings.push({ message });
			else
				receipt.harnessFindings.push({
					message,
					unclassifiedException: !(error instanceof HarnessInvalid),
				});
		}
		await sample("after-row");
		const [layout, elements, logs] = await Promise.all([
			diagnosticCall("layout", () =>
				driver.getLayoutInfo(
					{ target: { type: "main" } },
					{ timeoutMs: 6_000 },
				),
			),
			diagnosticCall("elements", () =>
				driver.getElements(
					{ target: { type: "main" }, limit: 1_000 },
					{ timeoutMs: 6_000 },
				),
			),
			diagnosticCall("logs", () =>
				driver.getLogs({ limit: 2_000 }, { timeoutMs: 6_000 }),
			),
		]);
		const freshErrors =
			logs && typeof logs === "object" && !("diagnosticError" in logs)
				? logEntries(logs as Json).filter(
						(entry) =>
							!baseline.has(logFingerprint(entry)) &&
							String(entry.level ?? "").toLowerCase() === "error",
					)
				: [];
		check("no_new_error_logs", freshErrors.length === 0, "product", {
			freshErrors,
		});
		const finishedAtMs = Date.now();
		const timings = {
			rowId,
			startedAtMs,
			finishedAtMs,
			durationMs: finishedAtMs - startedAtMs,
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
		await writeJson(join(rowDir, "receipt.json"), receipt);
		await writeJson(join(rowDir, "adjudication.json"), {
			rowId,
			classification: receipt.classification,
			productFindings: receipt.productFindings,
			harnessFindings: receipt.harnessFindings,
			environmentFindings: receipt.environmentFindings,
		});
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
	}

	const rowBodies: Record<RowId, Parameters<typeof executeRow>[1]> = {
		"tap-closed-instant": async (receipt, check, sample) => {
			await ensureHidden("tap-closed-instant");
			const started = performance.now();
			await gesture("down", "tap-closed-instant-down");
			const afterDown = await state();
			const mainAfterDown = await sampleMainWindow();
			const downObservedMs = Math.round(performance.now() - started);
			await sample("after-key-down", { downObservedMs, mainAfterDown });
			check(
				"keydown_shows_launcher",
				afterDown.windowVisible === true && afterDown.promptType === "none",
				"product",
				{
					downObservedMs,
					promptType: afterDown.promptType,
					windowVisible: afterDown.windowVisible,
				},
			);
			check(
				"main_window_identity_is_stable_id",
				mainAfterDown?.id === "main",
				"product",
				{ mainAfterDown },
			);
			await gesture("up", "tap-closed-instant-up");
			await Bun.sleep(DOUBLE_MS + 60);
			const settled = await state();
			check(
				"opening_tap_settles_on_launcher",
				settled.windowVisible === true && settled.promptType === "none",
				"product",
				{
					promptType: settled.promptType,
					windowVisible: settled.windowVisible,
				},
			);
			receipt.evidence.downObservedMs = downObservedMs;
		},
		"tap-open-empty-immediate": async (_receipt, check, sample) => {
			await openingTap("tap-open-empty-stage");
			await driver.setFilterAndWait("", { timeoutMs: 6_000 });
			await gesture("down", "tap-open-empty-down");
			await gesture("up", "tap-open-empty-up");
			await driver.waitForState({ windowVisible: false }, { timeoutMs: 2_000 });
			const afterPreview = await state();
			await sample("after-tap-preview", { afterPreview });
			check(
				"empty_launcher_tap_hides_without_double_window_wait",
				afterPreview.windowVisible === false,
				"product",
				{
					doubleWindowMs: DOUBLE_MS,
					windowVisible: afterPreview.windowVisible,
					promptType: afterPreview.promptType,
				},
			);
		},
		"tap-open-filtered-deferred": async (_receipt, check, sample) => {
			const marker = "nn31-filter-must-survive-preview";
			await openingTap("tap-open-filtered-stage");
			await driver.setFilterAndWait(marker, { timeoutMs: 6_000 });
			await gesture("down", "tap-open-filtered-down");
			await gesture("up", "tap-open-filtered-up");
			await Bun.sleep(100);
			const insideWindow = await state();
			await sample("inside-double-window", { insideWindow });
			check(
				"typed_launcher_does_not_preview_hide",
				insideWindow.windowVisible === true,
				"product",
				{
					sampledBeforeMs: DOUBLE_MS,
					windowVisible: insideWindow.windowVisible,
				},
			);
			check(
				"typed_filter_survives_double_window",
				insideWindow.inputValue === marker,
				"product",
				{
					expected: marker,
					actual: insideWindow.inputValue,
				},
			);
			await driver.waitForState(
				{ windowVisible: false },
				{ timeoutMs: DOUBLE_MS + 2_000 },
			);
			check(
				"typed_launcher_final_tap_hides",
				(await state()).windowVisible === false,
				"product",
			);
		},
		"hold-closed-day-page": async (_receipt, check, sample) => {
			await ensureHidden("hold-closed-day-page");
			await gesture("down", "hold-closed-down");
			await Bun.sleep(HOLD_MS + 100);
			const duringHold = await state();
			await sample("during-hold", { duringHold });
			check(
				"hold_from_closed_opens_day_page",
				duringHold.windowVisible === true &&
					duringHold.promptType === "dayPage",
				"product",
				{
					holdMarginMs: 100,
					promptType: duringHold.promptType,
					windowVisible: duringHold.windowVisible,
				},
			);
			await gesture("up", "hold-closed-up");
			await Bun.sleep(80);
			const afterRelease = await state();
			check(
				"hold_release_keeps_day_page",
				afterRelease.windowVisible === true &&
					afterRelease.promptType === "dayPage",
				"product",
				{
					promptType: afterRelease.promptType,
					windowVisible: afterRelease.windowVisible,
				},
			);
		},
		"hold-open-inert": async (_receipt, check, sample) => {
			await openingTap("hold-open-stage");
			await gesture("down", "hold-open-down");
			await Bun.sleep(HOLD_MS + 100);
			const duringHold = await state();
			await sample("during-open-hold", { duringHold });
			check(
				"hold_while_open_does_not_deepen",
				duringHold.windowVisible === true && duringHold.promptType === "none",
				"product",
				{
					contract: "intentionally dead/open question",
					promptType: duringHold.promptType,
					windowVisible: duringHold.windowVisible,
				},
			);
			await gesture("up", "hold-open-up");
			await Bun.sleep(80);
			const afterRelease = await state();
			check(
				"open_hold_release_is_inert",
				afterRelease.windowVisible === true &&
					afterRelease.promptType === "none",
				"product",
				{
					promptType: afterRelease.promptType,
					windowVisible: afterRelease.windowVisible,
				},
			);
		},
		"tap-open-day-page-immediate": async (_receipt, check, sample) => {
			await ensureHidden("tap-open-day-page");
			await gesture("down", "tap-open-day-page-hold-down");
			await Bun.sleep(HOLD_MS + 100);
			await gesture("up", "tap-open-day-page-hold-up");
			await driver.waitForState(
				{ windowVisible: true, promptType: "dayPage" },
				{ timeoutMs: 8_000 },
			);
			await gesture("down", "tap-open-day-page-tap-down");
			await gesture("up", "tap-open-day-page-tap-up");
			await driver.waitForState({ windowVisible: false }, { timeoutMs: 2_000 });
			const app = await state();
			await sample("after-day-page-tap-preview", { app });
			check(
				"tap_while_day_page_open_hides_immediately",
				app.windowVisible === false,
				"product",
				{
					doubleWindowMs: DOUBLE_MS,
					promptType: app.promptType,
					windowVisible: app.windowVisible,
				},
			);
		},
		"double-tap-closed-clean-chat": async (_receipt, check, sample) => {
			await ensureHidden("double-tap-closed");
			await gesture("down", "double-tap-first-down");
			await gesture("up", "double-tap-first-up");
			await Bun.sleep(60);
			await gesture("down", "double-tap-second-down");
			await gesture("up", "double-tap-second-up");
			let app = await state();
			const deadline = Date.now() + 8_000;
			while (
				Date.now() < deadline &&
				!String(app.promptType ?? "")
					.toLowerCase()
					.includes("agent")
			) {
				await Bun.sleep(40);
				app = await state();
			}
			const chat = await agentChatState();
			await sample("after-double-tap", { app, chat });
			check(
				"double_tap_opens_agent_chat",
				app.windowVisible === true &&
					String(app.promptType ?? "")
						.toLowerCase()
						.includes("agent"),
				"product",
				{
					promptType: app.promptType,
					windowVisible: app.windowVisible,
				},
			);
			check(
				"double_tap_uses_clean_quick_question_entry",
				Number(chat.contextChipCount ?? 0) === 0 &&
					String(chat.inputText ?? "") === "",
				"product",
				{
					contextChipCount: chat.contextChipCount,
					inputText: chat.inputText,
					status: chat.status,
				},
			);
			check(
				"double_tap_does_not_submit",
				Number(chat.messageCount ?? 0) === 0,
				"product",
				{
					messageCount: chat.messageCount,
				},
			);
		},
		"double-tap-open-after-preview": async (_receipt, check, sample) => {
			await openingTap("double-tap-open-stage");
			await driver.setFilterAndWait("", { timeoutMs: 6_000 });
			await gesture("down", "double-tap-open-first-down");
			await gesture("up", "double-tap-open-first-up");
			await Bun.sleep(60);
			await gesture("down", "double-tap-open-second-down");
			await gesture("up", "double-tap-open-second-up");
			let app = await state();
			const deadline = Date.now() + 8_000;
			while (
				Date.now() < deadline &&
				!String(app.promptType ?? "")
					.toLowerCase()
					.includes("agent")
			) {
				await Bun.sleep(40);
				app = await state();
			}
			const chat = await agentChatState();
			await sample("after-open-double-tap", { app, chat });
			check(
				"open_double_tap_promotes_after_preview",
				app.windowVisible === true &&
					String(app.promptType ?? "")
						.toLowerCase()
						.includes("agent"),
				"product",
				{
					promptType: app.promptType,
					windowVisible: app.windowVisible,
				},
			);
			check(
				"open_double_tap_still_uses_clean_entry",
				Number(chat.contextChipCount ?? 0) === 0 &&
					String(chat.inputText ?? "") === "",
				"product",
				{
					contextChipCount: chat.contextChipCount,
					inputText: chat.inputText,
				},
			);
		},
		"key-repeat-storm-single-hold": async (_receipt, check, sample) => {
			await ensureHidden("repeat-storm");
			const logsBefore = (await driver.getLogs(
				{ limit: 2_000 },
				{ timeoutMs: 6_000 },
			)) as Json;
			const beforeCount =
				JSON.stringify(logsBefore).split("HoldStart — show Day Page").length -
				1;
			await gesture("down", "repeat-storm-initial-down");
			for (let i = 0; i < 12; i++) {
				driver.send({
					type: "simulateMainHotkeyGesture",
					phase: "down",
					requestId: `${RUN_ID}-repeat-storm-${i}`,
				});
			}
			await Bun.sleep(HOLD_MS + 120);
			await gesture("up", "repeat-storm-up");
			await Bun.sleep(100);
			const app = await state();
			const logsAfter = (await driver.getLogs(
				{ limit: 2_000 },
				{ timeoutMs: 6_000 },
			)) as Json;
			const afterCount =
				JSON.stringify(logsAfter).split("HoldStart — show Day Page").length - 1;
			await sample("after-repeat-storm", {
				app,
				holdRouteDelta: afterCount - beforeCount,
			});
			check(
				"repeat_storm_routes_one_hold",
				app.promptType === "dayPage" && afterCount - beforeCount === 1,
				"product",
				{
					promptType: app.promptType,
					repeatedDownCount: 12,
					holdRouteDelta: afterCount - beforeCount,
				},
			);
			check(
				"repeat_storm_does_not_open_agent_chat",
				!String(app.promptType ?? "")
					.toLowerCase()
					.includes("agent"),
				"product",
				{
					promptType: app.promptType,
				},
			);
		},
		"space-first-day-page": async (_receipt, check, sample) => {
			await openingTap("space-first-stage");
			await driver.setFilterAndWait("", { timeoutMs: 6_000 });
			await driver.simulateGpuiKeyDown("space", {
				text: " ",
				target: { type: "main" },
				timeoutMs: 6_000,
			});
			await driver.waitForState(
				{ windowVisible: true, promptType: "dayPage" },
				{ timeoutMs: 8_000 },
			);
			const app = await state();
			await sample("after-space-first-char", { app });
			check(
				"space_first_character_opens_day_page",
				app.promptType === "dayPage",
				"product",
				{
					promptType: app.promptType,
				},
			);
			check(
				"space_trigger_is_cleared_not_editor_content",
				String(app.inputValue ?? "") === "",
				"product",
				{
					inputValue: app.inputValue,
				},
			);
		},
		"space-prefixed-query-control": async (_receipt, check, sample) => {
			const query = " todo";
			await openingTap("space-prefixed-control-stage");
			await driver.setFilterAndWait(query, { timeoutMs: 6_000 });
			await Bun.sleep(120);
			const app = await state();
			await sample("after-space-prefixed-query", { app });
			check(
				"space_prefixed_paste_stays_launcher_search",
				app.promptType === "none" && app.windowVisible === true,
				"product",
				{
					promptType: app.promptType,
					windowVisible: app.windowVisible,
				},
			);
			check(
				"space_prefixed_query_is_preserved",
				app.inputValue === query,
				"product",
				{
					expected: query,
					actual: app.inputValue,
				},
			);
		},
		"semicolon-first-capture-picker": async (receipt, check, sample) => {
			await openingTap("semicolon-first-stage");
			await driver.setFilterAndWait("", { timeoutMs: 6_000 });
			await driver.simulateGpuiKeyDown(";", {
				text: ";",
				target: { type: "main" },
				timeoutMs: 6_000,
			});
			await driver.waitForState({ inputValue: ";" }, { timeoutMs: 6_000 });
			await Bun.sleep(120);
			const app = await state();
			const elements = (await driver.getElements(
				{ target: { type: "main" }, limit: 1_000 },
				{ timeoutMs: 6_000 },
			)) as Json;
			const labels = collectLabels(elements);
			const expectedTargets = ["Todo", "Note", "Link", "Snippet"];
			const foundTargets = expectedTargets.filter((target) =>
				labels.some((label) =>
					label.toLowerCase().includes(target.toLowerCase()),
				),
			);
			await sample("after-semicolon-first-char", { app, foundTargets });
			check(
				"semicolon_first_character_keeps_main_surface",
				app.promptType === "none" && app.inputValue === ";",
				"product",
				{
					promptType: app.promptType,
					inputValue: app.inputValue,
				},
			);
			check(
				"semicolon_lists_canonical_capture_targets",
				foundTargets.length === expectedTargets.length,
				"product",
				{
					expectedTargets,
					foundTargets,
					labelSample: labels.slice(0, 80),
				},
			);
			receipt.evidence.foundTargets = foundTargets;
		},
		"semicolon-target-accept": async (_receipt, check, sample) => {
			await openingTap("semicolon-accept-stage");
			await driver.setFilterAndWait(";to", { timeoutMs: 6_000 });
			driver.simulateKey("enter");
			let app = await state();
			const deadline = Date.now() + 6_000;
			while (
				Date.now() < deadline &&
				!String(app.inputValue ?? "").startsWith("todo;")
			) {
				await Bun.sleep(40);
				app = await state();
			}
			const elements = (await driver.getElements(
				{ target: { type: "main" }, limit: 1_000 },
				{ timeoutMs: 6_000 },
			)) as Json;
			const labels = collectLabels(elements);
			const formIds = labels.filter((label) =>
				label.startsWith("handler-form:todo:"),
			);
			await sample("after-semicolon-target-accept", { app, formIds });
			check(
				"semicolon_target_accepts_to_postfix_composer",
				String(app.inputValue ?? "").startsWith("todo;"),
				"product",
				{
					inputValue: app.inputValue,
				},
			);
			check("todo_capture_form_owns_input", formIds.length > 0, "product", {
				formIds,
				labelSample: labels.slice(0, 100),
			});
		},
		"semicolon-ordinary-text-control": async (_receipt, check, sample) => {
			const query = "hello world; not a capture";
			await openingTap("semicolon-ordinary-stage");
			await driver.setFilterAndWait(query, { timeoutMs: 6_000 });
			await Bun.sleep(120);
			const app = await state();
			const elements = (await driver.getElements(
				{ target: { type: "main" }, limit: 1_000 },
				{ timeoutMs: 6_000 },
			)) as Json;
			const labels = collectLabels(elements);
			const formIds = labels.filter((label) =>
				label.startsWith("handler-form:"),
			);
			await sample("after-ordinary-semicolon-text", { app, formIds });
			check(
				"ordinary_semicolon_text_stays_search",
				app.promptType === "none" && app.inputValue === query,
				"product",
				{
					promptType: app.promptType,
					inputValue: app.inputValue,
				},
			);
			check(
				"ordinary_semicolon_text_does_not_open_capture_form",
				formIds.length === 0,
				"product",
				{
					formIds,
				},
			);
		},
		"escape-hide-resync": async (_receipt, check, sample) => {
			await openingTap("escape-resync-stage");
			await driver.setFilterAndWait("", { timeoutMs: 6_000 });
			driver.simulateKey(" ");
			await driver.waitForState(
				{ windowVisible: true, promptType: "dayPage" },
				{ timeoutMs: 8_000 },
			);
			driver.simulateKey("escape");
			await driver.waitForState({ windowVisible: false }, { timeoutMs: 8_000 });
			await gesture("down", "escape-resync-down");
			const afterDown = await state();
			await sample("after-resync-key-down", { afterDown });
			check(
				"escape_hide_resets_classifier_to_closed",
				afterDown.windowVisible === true && afterDown.promptType === "none",
				"product",
				{
					promptType: afterDown.promptType,
					windowVisible: afterDown.windowVisible,
				},
			);
			await gesture("up", "escape-resync-up");
			await Bun.sleep(DOUBLE_MS + 60);
			const settled = await state();
			check(
				"resynced_opening_tap_stays_launcher",
				settled.windowVisible === true && settled.promptType === "none",
				"product",
				{
					promptType: settled.promptType,
					windowVisible: settled.windowVisible,
				},
			);
		},
	};

	try {
		for (const row of rows) await executeRow(row, rowBodies[row]);
		await hide("final-cleanup");
		summary.cleanup = { hidden: (await state()).windowVisible === false };
	} finally {
		summary.sessionDir = driver.sessionDir;
		summary.appLog = driver.logPath;
		await driver.close();
		summary.driverFinalization = driver.finalization;
		const binaryPath = resolve(BINARY);
		const processPattern = `^${binaryPath.replace(/[\\^$.*+?()[\]{}|]/g, "\\$&")}([[:space:]]|$)`;
		const processProbe = Bun.spawnSync(["pgrep", "-f", processPattern], {
			stdout: "pipe",
			stderr: "pipe",
		});
		const stdout = new TextDecoder().decode(processProbe.stdout).trim();
		const processCheck = {
			command: ["pgrep", "-f", processPattern],
			exitCode: processProbe.exitCode,
			stdout,
			stderr: new TextDecoder().decode(processProbe.stderr).trim(),
			clean: processProbe.exitCode === 1 && stdout.length === 0,
		};
		summary.postTeardownProcessCheck = processCheck;
		await writeJson(
			join(OUTPUT_DIR, "post-teardown-process.json"),
			processCheck,
		);
		if (!processCheck.clean)
			summary.harnessFindings.push({
				rowId: "postTeardown",
				name: "binary_process_remained",
				detail: processCheck,
			});
	}

	summary.classification = summary.environmentFindings.length
		? "blocked-by-environment"
		: summary.harnessFindings.length
			? "invalid-harness"
			: summary.productFindings.length
				? "failed-product"
				: "verified";
	summary.pass = summary.classification === "verified";
	await writeJson(join(OUTPUT_DIR, "summary.json"), summary);
	console.log(JSON.stringify(summary, null, 2));
	process.exit(
		summary.pass
			? 0
			: summary.classification === "blocked-by-environment"
				? 3
				: summary.classification === "invalid-harness"
					? 2
					: 1,
	);
}

await runBattery();

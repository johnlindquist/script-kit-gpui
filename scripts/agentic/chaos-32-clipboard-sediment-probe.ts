#!/usr/bin/env bun
/// <reference types="bun-types" />
/**
 * NN=32 Clipboard History / sediment PREP battery.
 *
 * One paid sandbox launch covers every runtime-safe row. A tiny bootstrap
 * seeds Driver.launch({sandboxHome:true}) before exec'ing the pinned app; the
 * probe never reads or writes the process-global macOS pasteboard.
 *
 * Required-but-unsafe capture rows remain fail-closed BLOCKED until DevTools
 * exposes a synthetic clipboard-capture primitive that can provide payload,
 * source bundle id, and concealed-type flags without touching NSPasteboard.
 *
 * Usage after SCREEN/runtime assignment:
 *   PROBE_BINARY=target-agent/artifacts/finder-clipboard/script-kit-gpui \
 *   PROBE_OUTPUT_DIR=.test-output/chaos-32-clipboard-sediment/<lane-run> \
 *   bun scripts/agentic/chaos-32-clipboard-sediment-probe.ts
 *
 * Static fixture seeding (used by seeded-app-bootstrap.sh):
 *   bun scripts/agentic/chaos-32-clipboard-sediment-probe.ts \
 *     --seed-home /tmp/scratch-home --fixture <payloads.json>
 */

import { Database } from "bun:sqlite";
import {
	chmodSync,
	existsSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver.ts";

const PROJECT_ROOT = resolve(import.meta.dir, "../..");
const PROBE_PATH = resolve(import.meta.path);
const DEFAULT_FIXTURE = resolve(
	import.meta.dir,
	"fixtures/chaos-32-clipboard-sediment/payloads.json",
);
const BOOTSTRAP = resolve(
	import.meta.dir,
	"fixtures/chaos-32-clipboard-sediment/seeded-app-bootstrap.sh",
);
const BINARY = resolve(
	PROJECT_ROOT,
	process.env.PROBE_BINARY ??
		process.env.SCRIPT_KIT_GPUI_BINARY ??
		"target-agent/artifacts/finder-clipboard/script-kit-gpui",
);
const RUN_ID = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
const OUTPUT_DIR = resolve(
	PROJECT_ROOT,
	process.env.PROBE_OUTPUT_DIR ??
		`.test-output/chaos-32-clipboard-sediment/finder-clipboard-${RUN_ID}`,
);
const MAIN_TARGET: Json = { type: "main" };
const ACTIONS_TARGET: Json = { type: "kind", kind: "actionsDialog" };
const POST_COPY_AUTOMATION_ID = "clipboard-post-copy-menu";
const POST_COPY_SURFACE = "clipboardPostCopyMenu";

const ALL_ROWS = [
	"hostile-payloads",
	"binary-payloads",
	"rapid-dedupe-ordering",
	"search-matrix",
	"explicit-keep-no-popup",
	"capture-rejection",
	"auto-sediment",
] as const;
const RUNTIME_ROWS = ALL_ROWS.slice(0, 5);
type RowId = (typeof ALL_ROWS)[number];
type CheckKind = "product" | "harness" | "environment";
type Check = { name: string; ok: boolean; kind: CheckKind; detail: Json };
type Fixture = {
	schemaVersion: number;
	fixtureId: string;
	limits: {
		maxTextBytes: number;
		rapidEntryCount: number;
		expectedHistoryRows: number;
	};
	tokens: Record<string, string>;
	hostileText: Array<{ id: string; label: string; text: string }>;
	generatedText: Array<{
		id: string;
		label: string;
		prefix: string;
		fill: string;
		byteLength: number;
	}>;
	binary: Array<{
		id: string;
		label: string;
		blobKey: string;
		pngBase64: string | null;
		width: number;
		height: number;
	}>;
	blockedCaptureCases: Json[];
};
type StateSample = {
	at: string;
	label: string;
	state: Json | null;
	error?: string;
};
type RowReceipt = {
	schemaVersion: 1;
	nn: 32;
	rowId: RowId;
	startedAt: string;
	finishedAt?: string;
	classification?: string;
	checks: Check[];
	observations: Json;
	stateSamples: StateSample[];
};

class HarnessInvalid extends Error {}
class EnvironmentBlocked extends Error {}

function argValue(name: string): string | null {
	const index = process.argv.indexOf(name);
	return index >= 0 ? String(process.argv[index + 1] ?? "") : null;
}

function loadFixture(path = argValue("--fixture") || DEFAULT_FIXTURE): Fixture {
	let parsed: Fixture;
	try {
		parsed = JSON.parse(readFileSync(resolve(path), "utf8")) as Fixture;
	} catch (error) {
		throw new HarnessInvalid(`invalid fixture ${path}: ${String(error)}`);
	}
	if (parsed.schemaVersion !== 1 || !parsed.fixtureId) {
		throw new HarnessInvalid(`unsupported fixture: ${path}`);
	}
	return parsed;
}

function selectedRows(): RowId[] {
	const raw = argValue("--rows");
	if (!raw || raw === "all") return [...ALL_ROWS];
	if (raw === "runtime") return [...RUNTIME_ROWS];
	const values = [
		...new Set(
			raw
				.split(",")
				.map((value) => value.trim())
				.filter(Boolean),
		),
	];
	const unknown = values.filter((value) => !ALL_ROWS.includes(value as RowId));
	if (unknown.length > 0)
		throw new HarnessInvalid(`unknown --rows: ${unknown.join(",")}`);
	if (values.length === 0) throw new HarnessInvalid("--rows selected no rows");
	return values as RowId[];
}

function generatedText(spec: Fixture["generatedText"][number]): string {
	const prefixBytes = new TextEncoder().encode(spec.prefix).length;
	const fillBytes = new TextEncoder().encode(spec.fill).length;
	if (fillBytes !== 1 || prefixBytes > spec.byteLength) {
		throw new HarnessInvalid(`invalid generated text fixture ${spec.id}`);
	}
	return `${spec.prefix}${spec.fill.repeat(spec.byteLength - prefixBytes)}`;
}

function createSchema(db: Database): void {
	db.exec(`
    CREATE TABLE history (
      id TEXT PRIMARY KEY,
      content TEXT NOT NULL,
      content_hash TEXT,
      content_type TEXT NOT NULL DEFAULT 'text',
      timestamp INTEGER NOT NULL,
      pinned INTEGER DEFAULT 0,
      ocr_text TEXT,
      text_preview TEXT,
      image_width INTEGER,
      image_height INTEGER,
      byte_size INTEGER DEFAULT 0,
      brain_kept INTEGER NOT NULL DEFAULT 0,
      brain_tier INTEGER NOT NULL DEFAULT 0,
      copy_count INTEGER NOT NULL DEFAULT 1,
      kept_url_day TEXT
    );
    CREATE INDEX idx_timestamp ON history(timestamp DESC);
    CREATE INDEX idx_pinned_timestamp ON history(pinned DESC, timestamp DESC);
    CREATE INDEX idx_dedup ON history(content_type, content_hash);
  `);
}

function seedHome(home: string, fixturePath: string): Json {
	const fixture = loadFixture(fixturePath);
	const kitPath = join(resolve(home), ".scriptkit");
	const dbDir = join(kitPath, "db");
	const blobDir = join(kitPath, "clipboard", "blobs");
	mkdirSync(dbDir, { recursive: true });
	mkdirSync(blobDir, { recursive: true });
	const dbPath = join(dbDir, "clipboard-history.sqlite");
	if (existsSync(dbPath))
		throw new HarnessInvalid(`seed db already exists: ${dbPath}`);

	const db = new Database(dbPath, { create: true });
	createSchema(db);
	const insert = db.prepare(`
    INSERT INTO history (
      id, content, content_hash, content_type, timestamp, pinned, ocr_text,
      text_preview, image_width, image_height, byte_size,
      brain_kept, brain_tier, copy_count, kept_url_day
    ) VALUES (
      ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
    )
  `);
	const now = Date.now();
	const ids: string[] = [];
	const add = (entry: {
		id: string;
		content: string;
		contentType?: string;
		timestamp: number;
		pinned?: number;
		ocrText?: string | null;
		textPreview?: string | null;
		imageWidth?: number | null;
		imageHeight?: number | null;
		brainKept?: number;
		brainTier?: number;
		copyCount?: number;
		keptUrlDay?: string | null;
	}) => {
		const contentType = entry.contentType ?? "text";
		let preview = entry.textPreview;
		if (preview === undefined) {
			preview =
				contentType === "image"
					? null
					: Array.from(entry.content).slice(0, 100).join("");
		}
		insert.run(
			entry.id,
			entry.content,
			`fixture-hash:${entry.id}`,
			contentType,
			entry.timestamp,
			entry.pinned ?? 0,
			entry.ocrText ?? null,
			preview,
			entry.imageWidth ?? null,
			entry.imageHeight ?? null,
			new TextEncoder().encode(entry.content).length,
			entry.brainKept ?? 0,
			entry.brainTier ?? 0,
			entry.copyCount ?? 1,
			entry.keptUrlDay ?? null,
		);
		ids.push(entry.id);
	};

	const transaction = db.transaction(() => {
		add({
			id: "order-pinned",
			content: "NN32-PINNED-OLDER-BUT-FIRST",
			timestamp: now - 90_000,
			pinned: 1,
		});
		add({
			id: "order-newest",
			content: "NN32-NEWEST-UNPINNED",
			timestamp: now - 10,
		});
		add({
			id: "keep-explicit",
			content: fixture.tokens.keepExplicit,
			timestamp: now - 20,
		});
		add({
			id: "search-plain",
			content: fixture.tokens.plainSearch,
			timestamp: now - 30,
		});
		add({
			id: "search-case",
			content: fixture.tokens.caseSearch,
			timestamp: now - 40,
		});
		add({
			id: "dedupe-hot",
			content: fixture.tokens.dedupe,
			timestamp: now - 50,
			copyCount: 64,
		});

		fixture.binary.forEach((binary, index) => {
			add({
				id: binary.id,
				content: `blob:${binary.blobKey}`,
				contentType: "image",
				timestamp: now - 60 - index,
				ocrText: index === 0 ? fixture.tokens.ocrSearch : null,
				textPreview: null,
				imageWidth: binary.width,
				imageHeight: binary.height,
			});
			if (binary.pngBase64 !== null) {
				writeFileSync(
					join(blobDir, `${binary.blobKey}.png`),
					Buffer.from(binary.pngBase64, "base64"),
				);
			}
		});

		fixture.hostileText.forEach((hostile, index) => {
			add({
				id: hostile.id,
				content: hostile.text,
				timestamp: now - 100 - index,
			});
		});
		fixture.generatedText.forEach((generated, index) => {
			add({
				id: generated.id,
				content: generatedText(generated),
				timestamp: now - 200 - index,
			});
		});
		for (let index = 0; index < fixture.limits.rapidEntryCount; index += 1) {
			add({
				id: `rapid-${String(index).padStart(3, "0")}`,
				content: `NN32-RAPID-${String(index).padStart(3, "0")}-burst`,
				timestamp: now - 10_000 - index,
			});
		}
	});
	transaction();
	db.exec("PRAGMA wal_checkpoint(TRUNCATE)");
	const countRowsSql = "SELECT COUNT(*) AS n FROM history";
	const countContentSql =
		"SELECT COUNT(*) AS n FROM history WHERE content = ?1";
	const rowCount = Number((db.query(countRowsSql).get() as { n: number }).n);
	const forbiddenCounts = Object.fromEntries(
		["oversizeRejected", "concealedRejected", "passwordRejected"].map((key) => {
			const token = fixture.tokens[key];
			const count = Number(
				(db.query(countContentSql).get(token) as { n: number }).n,
			);
			return [key, count];
		}),
	);
	db.close();
	return {
		fixtureId: fixture.fixtureId,
		home: resolve(home),
		kitPath,
		dbPath,
		rowCount,
		expectedHistoryRows: fixture.limits.expectedHistoryRows,
		insertedIds: ids,
		forbiddenCounts,
	};
}

const seedTarget = argValue("--seed-home");
if (seedTarget) {
	const fixturePath = argValue("--fixture") || DEFAULT_FIXTURE;
	const seeded = seedHome(seedTarget, fixturePath);
	process.stdout.write(`${JSON.stringify(seeded)}\n`);
	process.exit(0);
}

mkdirSync(OUTPUT_DIR, { recursive: true });
chmodSync(BOOTSTRAP, 0o755);
const fixture = loadFixture();
const requestedRows = selectedRows();

function walk(node: unknown, out: Json[] = []): Json[] {
	if (!node || typeof node !== "object") return out;
	if (Array.isArray(node)) {
		for (const item of node) walk(item, out);
		return out;
	}
	const value = node as Json;
	if (typeof value.semanticId === "string" || typeof value.id === "string")
		out.push(value);
	for (const child of Object.values(value)) walk(child, out);
	return out;
}

function choiceRows(elements: Json | null): Json[] {
	return elements
		? walk(elements).filter((entry) => entry.type === "choice")
		: [];
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

function countOccurrences(haystack: string, needle: string): number {
	return needle ? haystack.split(needle).length - 1 : 0;
}

function readMarkdownTree(
	root: string,
): Array<{ path: string; content: string }> {
	if (!existsSync(root)) return [];
	const files: Array<{ path: string; content: string }> = [];
	const pending = [root];
	while (pending.length > 0) {
		const current = pending.pop();
		if (!current) continue;
		for (const entry of readdirSync(current, { withFileTypes: true })) {
			const path = join(current, entry.name);
			if (entry.isDirectory()) pending.push(path);
			else if (entry.isFile() && entry.name.endsWith(".md")) {
				files.push({ path, content: readFileSync(path, "utf8") });
			}
		}
	}
	return files;
}

async function writeJson(path: string, value: unknown): Promise<void> {
	await Bun.write(path, `${JSON.stringify(value, null, 2)}\n`);
}

async function poll<T>(
	label: string,
	read: () => Promise<T>,
	accept: (value: T) => boolean,
	timeoutMs = 7000,
): Promise<T> {
	const started = performance.now();
	let last: T | undefined;
	let lastError: unknown;
	while (performance.now() - started < timeoutMs) {
		try {
			last = await read();
			if (accept(last)) return last;
		} catch (error) {
			lastError = error;
		}
		await Bun.sleep(35);
	}
	throw new HarnessInvalid(
		`${label} not observable in ${timeoutMs}ms last=${JSON.stringify(last)} error=${String(lastError ?? "")}`,
	);
}

function classifyLaunchError(error: unknown): never {
	const message = error instanceof Error ? error.message : String(error);
	if (
		/did not become ready|timeout|operation not permitted|sandbox/i.test(
			message,
		)
	) {
		throw new EnvironmentBlocked(message);
	}
	throw new HarnessInvalid(message);
}

process.stderr.write(
	`[driver] binary: ${BINARY} (explicit real-app pin; bootstrap exec target)\n`,
);
let driver: Driver;
try {
	driver = await Driver.launch({
		binary: BOOTSTRAP,
		sandboxHome: true,
		sessionName: `finder-clipboard-nn32-${RUN_ID}`,
		readyTimeoutMs: 30_000,
		defaultTimeoutMs: 10_000,
		env: {
			SCRIPT_KIT_CLIPBOARD_PROBE_REAL_BINARY: BINARY,
			SCRIPT_KIT_CLIPBOARD_PROBE_SCRIPT: PROBE_PATH,
			SCRIPT_KIT_CLIPBOARD_PROBE_FIXTURE: DEFAULT_FIXTURE,
			SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
			SCRIPT_KIT_DISABLE_CLIPBOARD_MONITOR: "1",
			SCRIPT_KIT_BRAIN_TZ: process.env.SCRIPT_KIT_BRAIN_TZ ?? "America/Denver",
		},
	});
} catch (error) {
	let classification = "invalid-harness";
	try {
		classifyLaunchError(error);
	} catch (classified) {
		if (classified instanceof EnvironmentBlocked)
			classification = "blocked-by-environment";
		await writeJson(join(OUTPUT_DIR, "summary.json"), {
			schemaVersion: 1,
			tool: "chaos-32-clipboard-sediment-probe",
			nn: 32,
			classification,
			pass: false,
			launchError: String(classified),
			binary: BINARY,
			fixtureId: fixture.fixtureId,
			requestedRows,
		});
		throw classified;
	}
}

const sandboxHome = join(driver.sessionDir, "home");
const kitPath = join(sandboxHome, ".scriptkit");
const dbPath = join(kitPath, "db", "clipboard-history.sqlite");
const brainRoot = join(kitPath, "brain");
const rowReceipts: RowReceipt[] = [];
const aggregateProductFindings: Json[] = [];
const aggregateHarnessFindings: Json[] = [];
const aggregateEnvironmentFindings: Json[] = [];

async function safeRead(
	label: string,
	read: () => Promise<Json>,
): Promise<Json> {
	try {
		return await read();
	} catch (error) {
		return { __captureError: label, message: String(error) };
	}
}

async function sampleState(
	receipt: RowReceipt,
	label: string,
): Promise<Json | null> {
	try {
		const state = (await driver.getState({ timeoutMs: 8000 })) as Json;
		receipt.stateSamples.push({ at: new Date().toISOString(), label, state });
		return state;
	} catch (error) {
		receipt.stateSamples.push({
			at: new Date().toISOString(),
			label,
			state: null,
			error: String(error),
		});
		return null;
	}
}

function addCheck(
	receipt: RowReceipt,
	name: string,
	ok: boolean,
	kind: CheckKind,
	detail: Json = {},
): void {
	receipt.checks.push({ name, ok, kind, detail });
	if (ok) return;
	const finding = { rowId: receipt.rowId, name, detail };
	if (kind === "product") aggregateProductFindings.push(finding);
	else if (kind === "harness") aggregateHarnessFindings.push(finding);
	else aggregateEnvironmentFindings.push(finding);
}

function popupWindow(windows: Json): Json | null {
	const list = (windows.windows ?? []) as Json[];
	return (
		list.find(
			(window) =>
				window.automationId === POST_COPY_AUTOMATION_ID ||
				window.semanticSurface === POST_COPY_SURFACE,
		) ?? null
	);
}

function dbSnapshot(): Json {
	if (!existsSync(dbPath)) return { exists: false };
	const db = new Database(dbPath, { readonly: true });
	try {
		const totalsSql = `
      SELECT
        COUNT(*) AS total,
        SUM(CASE WHEN pinned != 0 THEN 1 ELSE 0 END) AS pinned,
        SUM(CASE WHEN brain_kept != 0 THEN 1 ELSE 0 END) AS brain_kept,
        SUM(CASE WHEN content_type = 'image' THEN 1 ELSE 0 END) AS images
      FROM history
    `;
		const orderedSql = `
      SELECT id, content_type, timestamp, pinned, copy_count, brain_kept,
             brain_tier, kept_url_day, text_preview, ocr_text, image_width,
             image_height, byte_size
      FROM history
      ORDER BY pinned DESC, timestamp DESC
      LIMIT 100
    `;
		const totals = db.query(totalsSql).get() as Json;
		const ordered = db.query(orderedSql).all() as Json[];
		return { exists: true, totals, ordered };
	} finally {
		db.close();
	}
}

function dbEntryByContent(content: string): Json | null {
	const db = new Database(dbPath, { readonly: true });
	try {
		return db.query(`
      SELECT id, content, copy_count, brain_kept, brain_tier, kept_url_day
      FROM history WHERE content = ?1
    `).get(content) as Json | null;
	} finally {
		db.close();
	}
}

async function captureBundle(receipt: RowReceipt): Promise<void> {
	const rowDir = join(OUTPUT_DIR, receipt.rowId);
	mkdirSync(rowDir, { recursive: true });
	await sampleState(receipt, "bundle-final");
	const [elements, layout, logs, windows] = await Promise.all([
		safeRead("elements", () =>
			driver.getElements(
				{ target: MAIN_TARGET, limit: 400 },
				{ timeoutMs: 8000 },
			),
		),
		safeRead("layout", () =>
			driver.getLayoutInfo({ target: MAIN_TARGET }, { timeoutMs: 8000 }),
		),
		safeRead("logs", () => driver.getLogs({ limit: 500 }, { timeoutMs: 8000 })),
		safeRead("windows", () =>
			driver.listAutomationWindows({ timeoutMs: 8000 }),
		),
	]);
	const timings = {
		startedAt: receipt.startedAt,
		finishedAt: receipt.finishedAt,
		driverStats: driver.stats,
	};
	await Promise.all([
		writeJson(join(rowDir, "receipt.json"), receipt),
		writeJson(join(rowDir, "elements.json"), elements),
		writeJson(join(rowDir, "layout.json"), layout),
		writeJson(join(rowDir, "app-logs.json"), logs),
		writeJson(join(rowDir, "state-samples.json"), receipt.stateSamples),
		writeJson(join(rowDir, "windows.json"), windows),
		writeJson(join(rowDir, "timings.json"), timings),
		writeJson(join(rowDir, "database.json"), dbSnapshot()),
	]);
}

async function runRow(
	rowId: RowId,
	body: (receipt: RowReceipt) => Promise<void> | void,
): Promise<void> {
	const receipt: RowReceipt = {
		schemaVersion: 1,
		nn: 32,
		rowId,
		startedAt: new Date().toISOString(),
		checks: [],
		observations: {},
		stateSamples: [],
	};
	try {
		await sampleState(receipt, "row-start");
		await body(receipt);
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		if (error instanceof EnvironmentBlocked)
			addCheck(receipt, "row_completed", false, "environment", { message });
		else addCheck(receipt, "row_completed", false, "harness", { message });
	} finally {
		receipt.finishedAt = new Date().toISOString();
		const productFailed = receipt.checks.some(
			(check) => check.kind === "product" && !check.ok,
		);
		const harnessFailed = receipt.checks.some(
			(check) => check.kind === "harness" && !check.ok,
		);
		const environmentFailed = receipt.checks.some(
			(check) => check.kind === "environment" && !check.ok,
		);
		if (productFailed) receipt.classification = "failed-product";
		else if (harnessFailed) receipt.classification = "invalid-harness";
		else if (environmentFailed) receipt.classification = "blocked";
		else receipt.classification = "verified";
		await captureBundle(receipt);
		rowReceipts.push(receipt);
	}
}

async function openClipboardHistory(): Promise<Json> {
	driver.send({ type: "triggerBuiltin", name: "clipboardHistory" });
	return poll(
		"clipboardHistory open",
		() => driver.getState({ timeoutMs: 8000 }),
		(state) => state.promptType === "clipboardHistory",
		10_000,
	);
}

async function filterCell(
	receipt: RowReceipt,
	cellId: string,
	filter: string,
): Promise<Json> {
	const cellStart = performance.now();
	await driver.setFilterAndWait(filter, { timeoutMs: 7000 });
	const settle = await driver
		.waitForSettle({ timeoutMs: 7000 })
		.catch((error) => ({ settled: false, error: String(error) }));
	const [state, elements] = await Promise.all([
		driver.getState({ timeoutMs: 8000 }),
		driver.getElements(
			{ target: MAIN_TARGET, limit: 400 },
			{ timeoutMs: 8000 },
		),
	]);
	receipt.stateSamples.push({
		at: new Date().toISOString(),
		label: `cell:${cellId}`,
		state,
	});
	const cell = {
		cellId,
		filter,
		elapsedMs: Math.round(performance.now() - cellStart),
		settle,
		state,
		rowCount: choiceRows(elements).length,
		elements,
	};
	const cellDir = join(OUTPUT_DIR, receipt.rowId, "cells");
	mkdirSync(cellDir, { recursive: true });
	await writeJson(join(cellDir, `${cellId}.json`), cell);
	return cell;
}

async function dialogState(): Promise<Json | null> {
	const windows = await driver.listAutomationWindows({ timeoutMs: 6000 });
	const list = (windows.windows ?? []) as Json[];
	const actions = list.find(
		(window) =>
			String(window.kind ?? "").toLowerCase() === "actionsdialog" ||
			String(window.semanticSurface ?? "").toLowerCase() === "actionsdialog",
	);
	return actions ?? null;
}

async function openActions(label: string): Promise<void> {
	const result = await driver.request(
		{
			type: "batch",
			requestId: `${RUN_ID}-open-actions-${label}-${Date.now()}`,
			target: MAIN_TARGET,
			commands: [{ type: "openActions" }],
			options: { stopOnError: true, timeout: 6000 },
		},
		{ expect: "batchResult", timeoutMs: 8000 },
	);
	if (result.success !== true)
		throw new HarnessInvalid(`openActions failed: ${JSON.stringify(result)}`);
	await poll(
		"ActionsDialog open",
		dialogState,
		(state) => Boolean(state),
		7000,
	);
}

async function activateKeepInToday(): Promise<Json> {
	await openActions("keep-in-today");
	const filterResult = await driver.request(
		{
			type: "batch",
			requestId: `${RUN_ID}-filter-actions-${Date.now()}`,
			target: ACTIONS_TARGET,
			commands: [{ type: "setInput", text: "Keep in Today" }],
			options: { stopOnError: true, timeout: 5000 },
		},
		{ expect: "batchResult", timeoutMs: 7000 },
	);
	if (filterResult.success !== true)
		throw new HarnessInvalid(
			`filter actions failed: ${JSON.stringify(filterResult)}`,
		);
	const isKeepAction = (id: string) =>
		id === "clip:clipboard_keep_in_today" || id === "clipboard_keep_in_today";
	const dialog = await poll(
		"Keep in Today action visible",
		dialogState,
		(state) => visibleActions(state).some((row) => isKeepAction(actionId(row))),
		6000,
	);
	const row = visibleActions(dialog).find((candidate) =>
		isKeepAction(actionId(candidate)),
	);
	const routedActionId = actionId(row ?? {});
	let semanticId = typeof row?.semanticId === "string" ? row.semanticId : null;
	if (!semanticId) {
		const elements = await driver.getElements(
			{ target: ACTIONS_TARGET, limit: 260 },
			{ timeoutMs: 6000 },
		);
		const node = walk(elements).find((candidate) =>
			String(candidate.semanticId ?? "").includes("clipboard_keep_in_today"),
		);
		semanticId = typeof node?.semanticId === "string" ? node.semanticId : null;
	}
	if (!semanticId || !isKeepAction(routedActionId)) {
		throw new HarnessInvalid("clipboard Keep in Today semantic row missing");
	}
	const selectResult = await driver.request(
		{
			type: "batch",
			requestId: `${RUN_ID}-select-keep-${Date.now()}`,
			target: ACTIONS_TARGET,
			commands: [{ type: "selectBySemanticId", semanticId }],
			options: { stopOnError: true, timeout: 5000 },
		},
		{ expect: "batchResult", timeoutMs: 7000 },
	);
	if (selectResult.success !== true)
		throw new HarnessInvalid(
			`select keep action failed: ${JSON.stringify(selectResult)}`,
		);
	await poll(
		"Keep in Today selected",
		dialogState,
		(state) => isKeepAction(selectedActionId(state) ?? ""),
		5000,
	);
	const dispatch = await driver.triggerAction(routedActionId, {
		host: "clipboardHistory",
		timeoutMs: 7_000,
	});
	await poll(
		"ActionsDialog close after Keep",
		dialogState,
		(state) => !state,
		7000,
	);
	return { semanticId, routedActionId, selectResult, dispatch };
}

function readKeepState(): Json | null {
	const db = new Database(dbPath, { readonly: true });
	try {
		const keepStateSql = `
      SELECT id, brain_kept, brain_tier, copy_count, kept_url_day
      FROM history WHERE id = 'keep-explicit'
    `;
		return db.query(keepStateSql).get() as Json | null;
	} finally {
		db.close();
	}
}

let bootstrapProof: Json = {};
try {
	const bootLog = existsSync(driver.logPath)
		? readFileSync(driver.logPath, "utf8")
		: "";
	bootstrapProof = {
		realBinary: BINARY,
		bootstrap: BOOTSTRAP,
		bootLogNamesRealBinary: bootLog.includes(BINARY),
		sandboxHome,
		fixtureId: fixture.fixtureId,
	};
	await openClipboardHistory();

	for (const rowId of requestedRows) {
		if (rowId === "hostile-payloads") {
			await runRow(rowId, async (receipt) => {
				const cell = await filterCell(receipt, "hostile-all", "NN32-");
				const db = dbSnapshot();
				const exactLimit = (db.ordered as Json[]).find(
					(row) => row.id === "hostile-limit",
				);
				addCheck(
					receipt,
					"hostile_rows_render",
					cell.rowCount >= 10,
					"product",
					{ rowCount: cell.rowCount },
				);
				addCheck(
					receipt,
					"exact_100kb_boundary_preserved",
					Number(exactLimit?.byte_size) === fixture.limits.maxTextBytes,
					"product",
					{ byteSize: exactLimit?.byte_size ?? null },
				);
				addCheck(receipt, "app_alive_after_hostiles", driver.alive, "product");
				receipt.observations = {
					cell: { filter: cell.filter, rowCount: cell.rowCount },
					db,
				};
			});
		} else if (rowId === "binary-payloads") {
			await runRow(rowId, async (receipt) => {
				const cell = await filterCell(receipt, "type-images", "type:images");
				const db = dbSnapshot();
				const images = (db.ordered as Json[]).filter(
					(row) => row.content_type === "image",
				);
				addCheck(
					receipt,
					"three_binary_variants_list",
					Number(cell.state.visibleChoiceCount) === 3 && images.length === 3,
					"product",
					{
						visibleChoiceCount: cell.state.visibleChoiceCount,
						dbImages: images.length,
					},
				);
				addCheck(
					receipt,
					"binary_metadata_survives",
					images.every(
						(row) =>
							Number(row.image_width) > 0 && Number(row.image_height) > 0,
					),
					"product",
					{ images },
				);
				addCheck(
					receipt,
					"app_alive_after_binary_rows",
					driver.alive,
					"product",
				);
				receipt.observations = {
					cell: { filter: cell.filter, rowCount: cell.rowCount },
					images,
				};
			});
		} else if (rowId === "rapid-dedupe-ordering") {
			await runRow(rowId, async (receipt) => {
				const rapidCell = await filterCell(receipt, "rapid-64", "NN32-RAPID-");
				const allCell = await filterCell(receipt, "all-order", "");
				const db = new Database(dbPath, { readonly: true });
				let ordered: Json[] = [];
				let dedupe: Json | null = null;
				try {
					const orderingSql =
						"SELECT id, pinned, timestamp FROM history ORDER BY pinned DESC, timestamp DESC LIMIT 8";
					const dedupeSql =
						"SELECT id, copy_count, COUNT(*) AS row_count FROM history WHERE content = ?1 GROUP BY id, copy_count";
					ordered = db.query(orderingSql).all() as Json[];
					dedupe = db
						.query(dedupeSql)
						.get(fixture.tokens.dedupe) as Json | null;
				} finally {
					db.close();
				}
				addCheck(
					receipt,
					"rapid_fire_64_rows_visible",
					Number(rapidCell.state.visibleChoiceCount) ===
						fixture.limits.rapidEntryCount,
					"product",
					{ visibleChoiceCount: rapidCell.state.visibleChoiceCount },
				);
				addCheck(
					receipt,
					"pinned_then_newest_order",
					ordered[0]?.id === "order-pinned" &&
						ordered[1]?.id === "order-newest",
					"product",
					{ ordered },
				);
				addCheck(
					receipt,
					"dedupe_state_projection",
					dedupe?.id === "dedupe-hot" &&
						Number(dedupe?.copy_count) === 64 &&
						Number(dedupe?.row_count) === 1,
					"product",
					{
						dedupe,
						scope: "seeded projection only; add_entry mechanism remains rung-3",
					},
				);
				addCheck(
					receipt,
					"dataset_count_matches_fixture",
					Number(allCell.state.choiceCount) ===
						fixture.limits.expectedHistoryRows,
					"product",
					{
						choiceCount: allCell.state.choiceCount,
						expected: fixture.limits.expectedHistoryRows,
					},
				);
				receipt.observations = {
					rapidCell: rapidCell.state,
					allCell: allCell.state,
					ordered,
					dedupe,
				};
			});
		} else if (rowId === "search-matrix") {
			await runRow(rowId, async (receipt) => {
				const cells: Record<string, Json> = {};
				cells.plain = await filterCell(
					receipt,
					"plain-contains",
					"FULLTEXT-LIGHTHOUSE",
				);
				cells.casefold = await filterCell(
					receipt,
					"case-insensitive",
					"mixed-case",
				);
				cells.ocr = await filterCell(receipt, "ocr-contains", "OCR-LIGHTHOUSE");
				cells.images = await filterCell(receipt, "type-images", "type:images");
				cells.texts = await filterCell(
					receipt,
					"type-texts",
					"type:texts NN32",
				);
				cells.zero = await filterCell(
					receipt,
					"zero-match",
					"NN32-NO-SUCH-ROW-XQZ",
				);
				cells.recovery = await filterCell(receipt, "clear-recovery", "");
				addCheck(
					receipt,
					"plain_contains_search",
					Number(cells.plain.state.visibleChoiceCount) === 1,
					"product",
					{ count: cells.plain.state.visibleChoiceCount },
				);
				addCheck(
					receipt,
					"case_insensitive_search",
					Number(cells.casefold.state.visibleChoiceCount) === 1,
					"product",
					{ count: cells.casefold.state.visibleChoiceCount },
				);
				addCheck(
					receipt,
					"ocr_search",
					Number(cells.ocr.state.visibleChoiceCount) === 1,
					"product",
					{ count: cells.ocr.state.visibleChoiceCount },
				);
				addCheck(
					receipt,
					"type_filter_images",
					Number(cells.images.state.visibleChoiceCount) === 3,
					"product",
					{ count: cells.images.state.visibleChoiceCount },
				);
				addCheck(
					receipt,
					"type_filter_texts",
					Number(cells.texts.state.visibleChoiceCount) >= 10,
					"product",
					{ count: cells.texts.state.visibleChoiceCount },
				);
				addCheck(
					receipt,
					"zero_then_recovery",
					Number(cells.zero.state.visibleChoiceCount) === 0 &&
						Number(cells.recovery.state.visibleChoiceCount) ===
							fixture.limits.expectedHistoryRows,
					"product",
					{
						zero: cells.zero.state.visibleChoiceCount,
						recovery: cells.recovery.state.visibleChoiceCount,
					},
				);
				receipt.observations = {
					implementationContract:
						"bounded case-insensitive contains over text_preview/OCR; not SQLite FTS",
					cells: Object.fromEntries(
						Object.entries(cells).map(([key, value]) => [
							key,
							{
								filter: value.filter,
								rowCount: value.rowCount,
								state: value.state,
								elapsedMs: value.elapsedMs,
							},
						]),
					),
				};
			});
		} else if (rowId === "explicit-keep-no-popup") {
			await runRow(rowId, async (receipt) => {
				const selected = await filterCell(
					receipt,
					"keep-target",
					fixture.tokens.keepExplicit,
				);
				addCheck(
					receipt,
					"keep_target_unique",
					Number(selected.state.visibleChoiceCount) === 1,
					"harness",
					{ count: selected.state.visibleChoiceCount },
				);
				const first = await activateKeepInToday();
				const firstState = await poll(
					"brain_kept after action",
					async () => readKeepState(),
					(state) => Number(state?.brain_kept) === 1,
					7000,
				);
				const firstFiles = readMarkdownTree(brainRoot);
				const uri = "kit://clipboard-history?id=keep-explicit";
				const joinedFirst = firstFiles.map((file) => file.content).join("\n");
				const windowsAfterFirst = await driver.listAutomationWindows({
					timeoutMs: 6000,
				});
				const second = await activateKeepInToday();
				await Bun.sleep(200);
				const secondState = readKeepState();
				const secondFiles = readMarkdownTree(brainRoot);
				const joinedSecond = secondFiles.map((file) => file.content).join("\n");
				const windowsAfterSecond = await driver.listAutomationWindows({
					timeoutMs: 6000,
				});
				addCheck(
					receipt,
					"keep_marks_sediment",
					Number(firstState?.brain_kept) === 1 &&
						Number(firstState?.brain_tier) > 0,
					"product",
					{ firstState },
				);
				addCheck(
					receipt,
					"day_page_uses_raw_free_reference",
					joinedFirst.includes(uri) &&
						joinedFirst.includes("[Clipboard entry](") &&
						!joinedFirst.includes(fixture.tokens.keepExplicit),
					"product",
					{ uri, markdownFiles: firstFiles.map((file) => file.path) },
				);
				addCheck(
					receipt,
					"keep_is_idempotent",
					countOccurrences(joinedSecond, uri) === 1 &&
						Number(secondState?.brain_kept) === 1,
					"product",
					{ uriOccurrences: countOccurrences(joinedSecond, uri), secondState },
				);
				addCheck(
					receipt,
					"no_fragment_created",
					secondFiles.every((file) => !file.path.includes("/fragments/")),
					"product",
					{ files: secondFiles.map((file) => file.path) },
				);
				addCheck(
					receipt,
					"post_copy_ui_absent",
					!popupWindow(windowsAfterFirst) && !popupWindow(windowsAfterSecond),
					"product",
					{
						afterFirst: popupWindow(windowsAfterFirst),
						afterSecond: popupWindow(windowsAfterSecond),
					},
				);
				receipt.observations = {
					first,
					second,
					firstState,
					secondState,
					uri,
					firstFiles,
					secondFiles,
					windowsAfterFirst,
					windowsAfterSecond,
				};
			});
		} else if (rowId === "capture-rejection") {
			await runRow(rowId, async (receipt) => {
				const prefixBytes = new TextEncoder().encode(fixture.tokens.oversizeRejected).length;
				const oversize = `${fixture.tokens.oversizeRejected}${"O".repeat(fixture.limits.maxTextBytes + 1 - prefixBytes)}`;
				const cases = {
					oversize: await driver.injectClipboardCaptureFixture({ text: oversize, sourceBundleId: "com.apple.TextEdit", changeGeneration: 32_003, timeoutMs: 8_000 }),
					concealed: await driver.injectClipboardCaptureFixture({ text: fixture.tokens.concealedRejected, concealedTypes: ["org.nspasteboard.ConcealedType"], changeGeneration: 32_011 }),
					passwordSource: await driver.injectClipboardCaptureFixture({ text: fixture.tokens.passwordSourceRejected, sourceBundleId: "com.1password.1password", changeGeneration: 32_012 }),
					secretShape: await driver.injectClipboardCaptureFixture({ text: fixture.tokens.passwordRejected, sourceBundleId: "com.apple.TextEdit", changeGeneration: 32_013 }),
				};
				const stored = {
					oversize: dbEntryByContent(oversize),
					concealed: dbEntryByContent(fixture.tokens.concealedRejected),
					passwordSource: dbEntryByContent(fixture.tokens.passwordSourceRejected),
					secretShape: dbEntryByContent(fixture.tokens.passwordRejected),
				};
				for (const [caseId, response] of Object.entries(cases)) addCheck(receipt, `${caseId}_protocol_echo`, response.type === "externalCommandResult" && response.ok === true, "harness", response);
				for (const [caseId, row] of Object.entries(stored)) addCheck(receipt, `${caseId}_rejected_before_storage`, row === null, "product", { row });
				receipt.observations = { cases, stored, realClipboardReadOrWrite: false };
			});
		} else if (rowId === "auto-sediment") {
			await runRow(rowId, async (receipt) => {
				const url = fixture.tokens.urlSediment;
				const nonUrl = fixture.tokens.nonUrlSediment;
				const urlFirst = await driver.injectClipboardCaptureFixture({ text: url, changeGeneration: 32_141 });
				const urlStateFirst = dbEntryByContent(url);
				const urlSecond = await driver.injectClipboardCaptureFixture({ text: url, changeGeneration: 32_142 });
				const urlStateSecond = dbEntryByContent(url);
				const nonUrlFirst = await driver.injectClipboardCaptureFixture({ text: nonUrl, changeGeneration: 32_151 });
				const nonUrlStateFirst = dbEntryByContent(nonUrl);
				const nonUrlSecond = await driver.injectClipboardCaptureFixture({ text: nonUrl, changeGeneration: 32_152 });
				const nonUrlStateSecond = dbEntryByContent(nonUrl);
				const markdown = readMarkdownTree(brainRoot);
				const joined = markdown.map((file) => file.content).join("\n");
				const urlUri = `kit://clipboard-history?id=${urlStateFirst?.id}`;
				const nonUrlUri = `kit://clipboard-history?id=${nonUrlStateFirst?.id}`;
				addCheck(receipt, "url_first_copy_auto_kept", Number(urlStateFirst?.brain_kept) === 1 && Number(urlStateFirst?.copy_count) === 1, "product", { urlStateFirst });
				addCheck(receipt, "url_recopy_dedupes_day_line", Number(urlStateSecond?.copy_count) === 2 && countOccurrences(joined, urlUri) === 1, "product", { urlStateSecond, urlUriOccurrences: countOccurrences(joined, urlUri) });
				addCheck(receipt, "non_url_first_copy_is_history_only", Number(nonUrlStateFirst?.brain_kept) === 0 && Number(nonUrlStateFirst?.copy_count) === 1, "product", { nonUrlStateFirst });
				addCheck(receipt, "non_url_recopy_promotes_once", Number(nonUrlStateSecond?.brain_kept) === 1 && Number(nonUrlStateSecond?.copy_count) === 2 && countOccurrences(joined, nonUrlUri) === 1, "product", { nonUrlStateSecond, nonUrlUriOccurrences: countOccurrences(joined, nonUrlUri) });
				for (const response of [urlFirst, urlSecond, nonUrlFirst, nonUrlSecond]) addCheck(receipt, `generation_${response.requestId}_echo`, response.type === "externalCommandResult" && response.ok === true, "harness", response);
				receipt.observations = { urlFirst, urlSecond, nonUrlFirst, nonUrlSecond, urlStateFirst, urlStateSecond, nonUrlStateFirst, nonUrlStateSecond, markdown, realClipboardReadOrWrite: false };
			});
		}
	}
} finally {
	driver.send({ type: "hide" });
	await Bun.sleep(150);
	const cleanupState = await safeRead("cleanup-state", () =>
		driver.getState({ timeoutMs: 4000 }),
	);
	await driver.close().catch(() => {});
	await Bun.sleep(100);
	const processCheck = Bun.spawnSync(["/bin/ps", "-axo", "pid=,command="], {
		stdout: "pipe",
		stderr: "pipe",
	});
	const processLines = processCheck.stdout
		.toString()
		.split("\n")
		.filter((line) => line.includes(BINARY));
	await writeJson(join(OUTPUT_DIR, "post-teardown-process.json"), {
		binary: BINARY,
		matchingProcesses: processLines,
		none: processLines.length === 0,
		cleanupState,
		finalization: driver.finalization,
	});
}

const productFailed = aggregateProductFindings.length > 0;
const harnessFailed = aggregateHarnessFindings.length > 0;
const blocked = aggregateEnvironmentFindings.length > 0;
let classification = "verified";
if (productFailed) classification = "failed-product";
else if (harnessFailed) classification = "invalid-harness";
else if (blocked) classification = "blocked";
const summary = {
	schemaVersion: 1,
	tool: "chaos-32-clipboard-sediment-probe",
	nn: 32,
	runId: RUN_ID,
	classification,
	pass: classification === "verified",
	binary: BINARY,
	bootstrapProof,
	fixtureId: fixture.fixtureId,
	requestedRows,
	rowClassifications: rowReceipts.map((receipt) => ({
		rowId: receipt.rowId,
		classification: receipt.classification,
	})),
	productFindings: aggregateProductFindings,
	harnessFindings: aggregateHarnessFindings,
	environmentFindings: aggregateEnvironmentFindings,
	outputDir: OUTPUT_DIR,
	safety: {
		sandboxHome,
		realClipboardReadOrWrite: false,
		nativeInput: false,
		network: false,
		screenshots: false,
	},
};
await writeJson(join(OUTPUT_DIR, "summary.json"), summary);
process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
let exitCode = 0;
if (productFailed) exitCode = 1;
else if (harnessFailed || blocked) exitCode = 2;
process.exit(exitCode);

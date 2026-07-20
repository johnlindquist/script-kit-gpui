#!/usr/bin/env bun
/** NN=28 OF-23 tranche 1: observed window choreography -> proposed contract. */
import {
	copyFileSync,
	existsSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";
import { openDayPage } from "./day-page-open-helper";

const ROOT = resolve(import.meta.dir, "../..");
const BINARY = resolve(
	ROOT,
	process.env.PROBE_BINARY ??
		process.env.SCRIPT_KIT_GPUI_BINARY ??
		"target-agent/artifacts/runner-2/script-kit-gpui",
);
const RUN_ID = process.env.NN28_RUN_ID ?? `runner-2-${Date.now().toString(36)}`;
const OUTPUT_DIR = resolve(
	ROOT,
	process.env.PROBE_OUTPUT_DIR ??
		`.test-output/chaos-28-window-choreography/${RUN_ID}`,
);
const MAIN: Json = { type: "main" };
const NOTES: Json = { type: "kind", kind: "notes", index: 0 };

type Obj = Record<string, any>;
type CheckKind = "product" | "harness" | "environment";
type Check = { name: string; ok: boolean; kind: CheckKind; detail: Json };
type RowReceipt = {
	rowId: string;
	startedAt: string;
	finishedAt?: string;
	classification: string;
	checks: Check[];
	observations: Obj;
	proposedContract: Obj;
};

class HarnessInvalid extends Error {}
class EnvironmentBlocked extends Error {}
mkdirSync(OUTPUT_DIR, { recursive: true });

function asObj(value: unknown): Obj {
	return value && typeof value === "object" && !Array.isArray(value)
		? (value as Obj)
		: {};
}

async function writeJson(path: string, value: unknown): Promise<void> {
	await Bun.write(path, `${JSON.stringify(value, null, 2)}\n`);
}

async function waitUntil<T>(
	label: string,
	read: () => Promise<T>,
	accept: (value: T) => boolean,
	timeoutMs = 8_000,
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

function windowRows(value: Json): Obj[] {
	return Array.isArray(value.windows) ? value.windows.map(asObj) : [];
}

function isNotesWindow(value: Obj): boolean {
	return (
		String(value.kind ?? "").toLowerCase() === "notes" || value.id === "notes"
	);
}

function windowById(value: Json, id: string): Obj | null {
	return windowRows(value).find((window) => window.id === id) ?? null;
}

function notesWindow(value: Json): Obj | null {
	return windowRows(value).find(isNotesWindow) ?? null;
}

function launcherSnapshot(state: Obj, elements: Obj, windows: Obj): Obj {
	const preflight = asObj(state.mainWindowPreflight);
	const scroll = asObj(state.mainListScroll);
	return {
		promptType: state.promptType,
		windowVisible: state.windowVisible,
		inputValue: state.inputValue,
		selectedIndex: state.selectedIndex,
		selectedResultKey: preflight.selectedResultKey ?? null,
		scrollTop: scroll.scrollTop ?? null,
		scrollTopItem: scroll.scrollTopItem ?? null,
		scrollTopOffset: scroll.scrollTopOffset ?? null,
		focusedSemanticId: elements.focusedSemanticId ?? null,
		focusedWindowId: windows.focusedWindowId ?? null,
	};
}

function notesSnapshot(state: Obj, elements: Obj, windows: Obj): Obj {
	const notes = asObj(state.notes);
	return {
		window: notesWindow(windows),
		focusedWindowId: windows.focusedWindowId ?? null,
		focusedSemanticId: elements.focusedSemanticId ?? null,
		activeNoteId: notes.activeNoteId ?? null,
		selectedNote: notes.selectedNote ?? null,
		dirtyState: notes.dirtyState ?? null,
		draftSnapshot: notes.draftSnapshot ?? null,
		editor: notes.editor ?? null,
		view: notes.view ?? null,
		focusTransitions: notes.focusTransitions ?? null,
		storage: notes.storage ?? null,
	};
}

async function mainState(driver: Driver): Promise<Obj> {
	return asObj(await driver.getState({ timeoutMs: 8_000 }));
}

async function notesState(driver: Driver): Promise<Obj> {
	return asObj(
		await driver.request(
			{ type: "getState", target: NOTES },
			{ expect: "stateResult", timeoutMs: 8_000 },
		),
	);
}

async function listWindows(driver: Driver): Promise<Obj> {
	return asObj(await driver.listAutomationWindows({ timeoutMs: 8_000 }));
}

async function safe<T>(read: () => Promise<T>): Promise<T | Obj> {
	try {
		return await read();
	} catch (error) {
		return { captureError: String(error) };
	}
}

async function capturePoint(driver: Driver, label: string): Promise<Obj> {
	const windows = await listWindows(driver);
	const hasNotes = Boolean(notesWindow(windows));
	const [main, mainElements, notes, notesElements] = await Promise.all([
		safe(() => mainState(driver)),
		safe(() =>
			driver.getElements({ target: MAIN, limit: 1_000 }, { timeoutMs: 8_000 }),
		),
		hasNotes ? safe(() => notesState(driver)) : Promise.resolve(null),
		hasNotes
			? safe(() =>
					driver.getElements(
						{ target: NOTES, limit: 1_000 },
						{ timeoutMs: 8_000 },
					),
				)
			: Promise.resolve(null),
	]);
	return {
		label,
		capturedAt: new Date().toISOString(),
		windows,
		main,
		mainElements,
		notes,
		notesElements,
	};
}

async function showMain(driver: Driver, label: string): Promise<void> {
	driver.send({ type: "show", requestId: `${RUN_ID}-${label}-show` });
	await waitUntil(
		`${label} main visible`,
		() => listWindows(driver),
		(windows) => windowById(windows, "main")?.visible === true,
	);
}

async function openNotesProtocol(driver: Driver, label: string): Promise<Obj> {
	driver.send({
		type: "openNotes",
		requestId: `${RUN_ID}-${label}-open-notes`,
	});
	const firstWindows = await waitUntil(
		`${label} Notes open`,
		() => listWindows(driver),
		(windows) => notesWindow(windows)?.visible === true,
		10_000,
	);
	const firstState = await notesState(driver);
	const firstElements = asObj(
		await driver.getElements(
			{ target: NOTES, limit: 1_000 },
			{ timeoutMs: 8_000 },
		),
	);
	const settle = await driver.waitForSettle({
		timeoutMs: 5_000,
		probe: async () => notesState(driver),
	});
	return {
		firstVisible: notesSnapshot(firstState, firstElements, firstWindows),
		settle,
		settled: await capturePoint(driver, `${label}:settled`),
	};
}

async function notesOpen(driver: Driver): Promise<boolean> {
	return Boolean(notesWindow(await listWindows(driver)));
}

async function ensureNotesClosed(driver: Driver, label: string): Promise<void> {
	if (!(await notesOpen(driver))) return;
	driver.send({
		type: "openNotes",
		requestId: `${RUN_ID}-${label}-toggle-close`,
	});
	await waitUntil(
		`${label} Notes closed`,
		() => listWindows(driver),
		(windows) => !notesWindow(windows),
		10_000,
	);
}

async function stageLauncher(driver: Driver, label: string): Promise<Obj> {
	await ensureNotesClosed(driver, `${label}-pre`);
	await showMain(driver, label);
	for (let attempt = 0; attempt < 3; attempt += 1) {
		const state = await mainState(driver);
		if (state.promptType === "none") break;
		driver.simulateKey("escape");
		await Bun.sleep(120);
	}
	await driver.setFilterAndWait("a", { timeoutMs: 8_000 });
	for (let index = 0; index < 12; index += 1) driver.simulateKey("down");
	await Bun.sleep(180);
	const state = await mainState(driver);
	const elements = asObj(
		await driver.getElements(
			{ target: MAIN, limit: 1_000 },
			{ timeoutMs: 8_000 },
		),
	);
	const windows = await listWindows(driver);
	return launcherSnapshot(state, elements, windows);
}

function findSemanticNode(value: unknown, semanticId: string): Obj | null {
	if (Array.isArray(value)) {
		for (const item of value) {
			const found = findSemanticNode(item, semanticId);
			if (found) return found;
		}
		return null;
	}
	const object = asObj(value);
	if (!Object.keys(object).length) return null;
	if (String(object.semanticId ?? object.id ?? "") === semanticId)
		return object;
	for (const child of Object.values(object)) {
		const found = findSemanticNode(child, semanticId);
		if (found) return found;
	}
	return null;
}

async function activateLauncherBuiltin(
	driver: Driver,
	builtinId: string,
	query: string,
	label: string,
): Promise<Obj> {
	await ensureNotesClosed(driver, `${label}-pre`);
	await showMain(driver, label);
	for (let attempt = 0; attempt < 3; attempt += 1) {
		const state = await mainState(driver);
		if (state.promptType === "none") break;
		driver.simulateKey("escape");
		await Bun.sleep(120);
	}
	await driver.setFilterAndWait(query, { timeoutMs: 8_000 });
	const selectedState = await waitUntil(
		`${label} exact launcher result`,
		() => mainState(driver),
		(state) => asObj(state.mainWindowPreflight).selectedResultKey === builtinId,
		8_000,
	);
	const selectedResultKey =
		asObj(selectedState.mainWindowPreflight).selectedResultKey ?? null;
	if (selectedResultKey !== builtinId) {
		throw new HarnessInvalid(
			`${label} selected ${String(selectedResultKey)} instead of ${builtinId}`,
		);
	}
	const elements = asObj(
		await driver.getElements(
			{ target: MAIN, limit: 1_000 },
			{ timeoutMs: 8_000 },
		),
	);
	const selectedSemanticId = String(elements.selectedSemanticId ?? "");
	const selectedNode = findSemanticNode(elements, selectedSemanticId);
	if (!selectedSemanticId || !selectedNode) {
		throw new HarnessInvalid(
			`${label} exact ${builtinId} selection lacked an observable semantic row`,
		);
	}
	const windows = await listWindows(driver);
	const launcherBefore = launcherSnapshot(selectedState, elements, windows);
	const activation = asObj(
		await driver.batch(
			[
				{
					type: "selectBySemanticId",
					semanticId: selectedSemanticId,
					submit: true,
				},
			],
			{ timeoutMs: 8_000, stopOnError: true },
		),
	);
	if (activation.success !== true) {
		throw new HarnessInvalid(
			`${label} launcher activation failed: ${JSON.stringify(activation)}`,
		);
	}
	return {
		builtinId,
		query,
		selectedResultKey,
		selectedSemanticId,
		selectedNode,
		launcherBefore,
		activation,
	};
}

async function closeNotesVariant(
	driver: Driver,
	variant: "escape" | "cmd-w" | "toggle",
	label: string,
): Promise<Obj> {
	let dispatch: Json = {};
	if (variant === "toggle") {
		driver.send({ type: "openNotes", requestId: `${RUN_ID}-${label}-toggle` });
	} else {
		dispatch = await driver.simulateGpuiEvent(
			{
				type: "keyDown",
				key: variant === "escape" ? "escape" : "w",
				modifiers: variant === "cmd-w" ? ["cmd"] : [],
			},
			{ target: NOTES, timeoutMs: 8_000 },
		);
	}
	const closedWindows = await waitUntil(
		`${label} ${variant} close`,
		() => listWindows(driver),
		(windows) => !notesWindow(windows),
		10_000,
	);
	return {
		variant,
		dispatch,
		closedWindows,
		after: await capturePoint(driver, `${label}:${variant}:after`),
	};
}

function sameLauncherState(before: Obj, after: Obj): Obj {
	return {
		inputValue: before.inputValue === after.inputValue,
		selectedIndex: before.selectedIndex === after.selectedIndex,
		selectedResultKey: before.selectedResultKey === after.selectedResultKey,
		scrollTop: before.scrollTop === after.scrollTop,
		scrollTopItem: before.scrollTopItem === after.scrollTopItem,
		scrollTopOffset: before.scrollTopOffset === after.scrollTopOffset,
	};
}

async function t01t06(
	driver: Driver,
	receipt: RowReceipt,
	check: (name: string, ok: boolean, kind: CheckKind, detail?: Json) => void,
): Promise<void> {
	const variants: Obj[] = [];
	for (const variant of ["escape", "cmd-w", "toggle"] as const) {
		const before = await stageLauncher(driver, `t01-t06-${variant}`);
		const opened = await openNotesProtocol(driver, `t01-t06-${variant}`);
		check(
			`${variant}_notes_opened`,
			Boolean(asObj(opened.firstVisible).window),
			"product",
			opened,
		);
		const closed = await closeNotesVariant(
			driver,
			variant,
			`t01-t06-${variant}`,
		);
		const afterPoint = asObj(closed.after);
		const afterMain = asObj(afterPoint.main);
		const afterElements = asObj(afterPoint.mainElements);
		const afterWindows = asObj(afterPoint.windows);
		const after = launcherSnapshot(afterMain, afterElements, afterWindows);
		variants.push({
			variant,
			before,
			opened,
			closed,
			after,
			preserved: sameLauncherState(before, after),
		});
	}
	receipt.observations = { variants };
	receipt.proposedContract = {
		entry:
			"protocol openNotes focuses Notes editor on the first observable Notes frame",
		closeVariants: variants.map((variant) => ({
			variant: variant.variant,
			launcherVisible: variant.after.windowVisible,
			launcherFocusedWindowId: variant.after.focusedWindowId,
			preserved: variant.preserved,
		})),
	};
}

async function t02(
	driver: Driver,
	receipt: RowReceipt,
	check: (name: string, ok: boolean, kind: CheckKind, detail?: Json) => void,
): Promise<void> {
	const activation = await activateLauncherBuiltin(
		driver,
		"builtin/open-notes",
		"Open Notes",
		"t02-open-notes",
	);
	const launcherBefore = asObj(activation.launcherBefore);
	const firstWindows = await waitUntil(
		"T02 builtin Notes open",
		() => listWindows(driver),
		(windows) => notesWindow(windows)?.visible === true,
		10_000,
	);
	const firstState = await notesState(driver);
	const firstElements = asObj(
		await driver.getElements(
			{ target: NOTES, limit: 1_000 },
			{ timeoutMs: 8_000 },
		),
	);
	const firstVisible = notesSnapshot(firstState, firstElements, firstWindows);
	check(
		"builtin_open_notes_opened",
		Boolean(firstVisible.window),
		"product",
		firstVisible,
	);
	const closed = await closeNotesVariant(driver, "escape", "t02");
	const afterPoint = asObj(closed.after);
	const after = launcherSnapshot(
		asObj(afterPoint.main),
		asObj(afterPoint.mainElements),
		asObj(afterPoint.windows),
	);
	receipt.observations = {
		activation,
		launcherBefore,
		firstVisible,
		closed,
		after,
	};
	receipt.proposedContract = {
		entry: {
			focusedWindowId: firstVisible.focusedWindowId,
			focusedSemanticId: firstVisible.focusedSemanticId,
			focusSurface: asObj(firstVisible.view).focusSurface ?? null,
		},
		afterClose: {
			launcherVisible: after.windowVisible,
			focusedWindowId: after.focusedWindowId,
			preserved: sameLauncherState(launcherBefore, after),
		},
	};
}

async function t03(
	driver: Driver,
	receipt: RowReceipt,
	check: (name: string, ok: boolean, kind: CheckKind, detail?: Json) => void,
): Promise<void> {
	const variants: Obj[] = [];
	for (const variant of [
		{
			id: "builtin/search-notes",
			label: "search-notes",
			query: "Search Notes",
		},
		{ id: "builtin/new-note", label: "new-note", query: "Create Note" },
		{
			id: "builtin/quick-capture",
			label: "quick-capture",
			query: "Quick Note Capture",
		},
	]) {
		const activation = await activateLauncherBuiltin(
			driver,
			variant.id,
			variant.query,
			`t03-${variant.label}`,
		);
		const launcherBefore = asObj(activation.launcherBefore);
		let firstWindows: Obj;
		try {
			firstWindows = await waitUntil(
				`T03 ${variant.label} Notes open`,
				() => listWindows(driver),
				(windows) => notesWindow(windows)?.visible === true,
				10_000,
			);
		} catch (error) {
			check(`${variant.label}_opened`, false, "product", {
				error: String(error),
			});
			variants.push({
				variant,
				activation,
				launcherBefore,
				openError: String(error),
			});
			continue;
		}
		const firstState = await notesState(driver);
		const firstElements = asObj(
			await driver.getElements(
				{ target: NOTES, limit: 1_000 },
				{ timeoutMs: 8_000 },
			),
		);
		const firstVisible = notesSnapshot(firstState, firstElements, firstWindows);
		check(`${variant.label}_opened`, true, "product", firstVisible);
		const firstEscape = await driver.simulateGpuiEvent(
			{ type: "keyDown", key: "escape", modifiers: [] },
			{ target: NOTES, timeoutMs: 8_000 },
		);
		await Bun.sleep(250);
		const afterFirstEscape = await capturePoint(
			driver,
			`t03:${variant.label}:after-first-escape`,
		);
		let secondEscape: Obj | null = null;
		if (await notesOpen(driver)) {
			const dispatch = await driver.simulateGpuiEvent(
				{ type: "keyDown", key: "escape", modifiers: [] },
				{ target: NOTES, timeoutMs: 8_000 },
			);
			await Bun.sleep(250);
			secondEscape = {
				dispatch,
				after: await capturePoint(
					driver,
					`t03:${variant.label}:after-second-escape`,
				),
			};
		}
		variants.push({
			variant,
			activation,
			launcherBefore,
			firstVisible,
			firstEscape,
			afterFirstEscape,
			secondEscape,
		});
		await ensureNotesClosed(driver, `t03-${variant.label}-cleanup`);
	}
	receipt.observations = { variants };
	receipt.proposedContract = {
		variants: variants.map((entry) => {
			const firstPoint = asObj(entry.afterFirstEscape);
			const firstNotes = asObj(firstPoint.notes);
			const firstWindows = asObj(firstPoint.windows);
			return {
				builtinId: entry.variant.id,
				entryFocusSurface: asObj(entry.firstVisible?.view).focusSurface ?? null,
				entryView: entry.firstVisible?.view ?? null,
				afterFirstEscapeNotesOpen: Boolean(notesWindow(firstWindows)),
				afterFirstEscapeView: firstNotes.notes?.view ?? firstNotes.view ?? null,
				afterSecondEscape: entry.secondEscape?.after ?? null,
			};
		}),
	};
}

function todayLocalDate(): string {
	const now = new Date();
	return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
}

function markerInCanonicalNotes(marker: string): Obj {
	const notesDir = join(sandboxHome, ".scriptkit", "brain", "notes");
	const files = existsSync(notesDir)
		? readdirSync(notesDir)
				.filter((name) => name.endsWith(".md"))
				.map((name) => join(notesDir, name))
		: [];
	const matches = files.filter((path) =>
		readFileSync(path, "utf8").includes(marker),
	);
	return { notesDir, fileCount: files.length, matchedFiles: matches };
}

async function t04t05(
	driver: Driver,
	receipt: RowReceipt,
	check: (name: string, ok: boolean, kind: CheckKind, detail?: Json) => void,
): Promise<void> {
	await stageLauncher(driver, "t04-t05");
	const opened = await openNotesProtocol(driver, "t04-t05-existing-notes");
	const beforeWindows = asObj(asObj(opened.settled).windows);
	const existingWindow = notesWindow(beforeWindows);
	const beforeState = await notesState(driver);
	const beforeNotes = asObj(beforeState.notes);
	const priorNoteId =
		String(
			beforeNotes.activeNoteId ?? asObj(beforeNotes.selectedNote).id ?? "",
		) || null;
	const dirtyMarker = `# NN28 dirty before external select ${RUN_ID}\n\nunsaved marker ${RUN_ID}`;
	const setDirty = asObj(
		await driver.request(
			{
				type: "batch",
				target: NOTES,
				requestId: `${RUN_ID}-t04-t05-set-dirty`,
				commands: [{ type: "setInput", text: dirtyMarker }],
				options: { stopOnError: true, timeout: 8_000 },
			},
			{ expect: "batchResult", timeoutMs: 10_000 },
		),
	);
	check("dirty_note_input_set", setDirty.success === true, "harness", setDirty);
	await waitUntil(
		"T05 dirty note observable",
		() => notesState(driver),
		(state) =>
			Number(asObj(state.notes).editor?.textLength ?? -1) ===
			dirtyMarker.length,
		8_000,
	);
	const dirtyState = await capturePoint(driver, "t04-t05:dirty-existing-note");

	const daysDir = join(sandboxHome, ".scriptkit", "brain", "days");
	mkdirSync(daysDir, { recursive: true });
	const dayText = `# ${todayLocalDate()}\n\nNN28 day handoff ${RUN_ID}\n`;
	writeFileSync(join(daysDir, `${todayLocalDate()}.md`), dayText);
	const dayPageBefore = await openDayPage(driver, RUN_ID);
	check(
		"day_page_opened_with_notes_existing",
		dayPageBefore.promptType === "dayPage",
		"product",
		dayPageBefore,
	);
	const beforeAction = await capturePoint(
		driver,
		"t04-t05:before-day-page-action",
	);
	const action = asObj(
		await driver.request(
			{
				type: "triggerAction",
				actionId: "day_page:open_in_notes_window",
				host: "mainList",
				requestId: `${RUN_ID}-t04-t05-open-in-notes`,
			},
			{ expect: "triggerActionResult", timeoutMs: 10_000 },
		),
	);
	check(
		"day_page_open_in_notes_action_ok",
		action.ok === true,
		"product",
		action,
	);
	await Bun.sleep(350);
	const afterAction = await capturePoint(
		driver,
		"t04-t05:after-day-page-action",
	);
	const afterWindows = asObj(afterAction.windows);
	const afterWindow = notesWindow(afterWindows);
	const afterNotesState = asObj(afterAction.notes);
	const afterNotes = asObj(afterNotesState.notes);
	const selectedAfter =
		String(
			afterNotes.activeNoteId ?? asObj(afterNotes.selectedNote).id ?? "",
		) || null;
	const sameWindow = Boolean(
		existingWindow?.id && existingWindow.id === afterWindow?.id,
	);
	const persistedPrior = markerInCanonicalNotes(`unsaved marker ${RUN_ID}`);
	const priorSaved = persistedPrior.matchedFiles.length === 1;
	check("existing_notes_window_reused", sameWindow, "product", {
		existingWindow,
		afterWindow,
	});
	check(
		"external_select_changed_note",
		Boolean(selectedAfter && selectedAfter !== priorNoteId),
		"product",
		{
			priorNoteId,
			selectedAfter,
		},
	);
	check(
		"dirty_prior_note_saved_before_external_select",
		priorSaved,
		"product",
		persistedPrior,
	);
	const closed = await closeNotesVariant(driver, "escape", "t04-t05");
	const returnPoint = asObj(closed.after);
	const returnMain = asObj(returnPoint.main);
	const returnWindows = asObj(returnPoint.windows);
	receipt.observations = {
		opened,
		existingWindow,
		dirtyState,
		priorNoteId,
		dayPageBefore,
		beforeAction,
		action,
		afterAction,
		selectedAfter,
		sameWindow,
		persistedPrior,
		closed,
	};
	receipt.proposedContract = {
		existingWindowReuse: sameWindow,
		externalSelectFromDayPage: {
			focusedWindowId: afterWindows.focusedWindowId ?? null,
			selectedNoteChanged: selectedAfter !== priorNoteId,
			priorDirtyNoteSaved: priorSaved,
		},
		afterNotesClose: {
			mainPromptType: returnMain.promptType ?? null,
			mainVisible: returnMain.windowVisible ?? null,
			focusedWindowId: returnWindows.focusedWindowId ?? null,
			dayPageInputValue: returnMain.inputValue ?? null,
		},
	};
}

async function d02(
	driver: Driver,
	receipt: RowReceipt,
	check: (name: string, ok: boolean, kind: CheckKind, detail?: Json) => void,
): Promise<void> {
	const launcherBefore = await stageLauncher(driver, "d02");
	const opened = await openNotesProtocol(driver, "d02-notes-open");
	const baselineText = `NN28 notes baseline ${RUN_ID}`;
	const setBaseline = asObj(
		await driver.request(
			{
				type: "batch",
				target: NOTES,
				requestId: `${RUN_ID}-d02-notes-baseline`,
				commands: [{ type: "setInput", text: baselineText }],
				options: { stopOnError: true, timeout: 8_000 },
			},
			{ expect: "batchResult", timeoutMs: 10_000 },
		),
	);
	check(
		"d02_notes_baseline_set",
		setBaseline.success === true,
		"harness",
		setBaseline,
	);
	await waitUntil(
		"D02 Notes baseline observable",
		() => notesState(driver),
		(state) =>
			Number(asObj(state.notes).editor?.textLength ?? -1) ===
			baselineText.length,
	);
	await showMain(driver, "d02-main-focus");
	const mainMarker = `NN28-MAIN-FOCUS-${RUN_ID}`;
	await driver.setFilterAndWait(mainMarker, { timeoutMs: 8_000 });
	const beforeDelivery = await capturePoint(
		driver,
		"d02:before-delivery-main-focused-notes-open",
	);
	const beforeMain = asObj(beforeDelivery.main);
	const beforeDictation = asObj(beforeMain.dictation);
	const beforeGeneration = Number(
		asObj(beforeDictation.lastDelivery).generation ?? 0,
	);
	const beforeNotesState = asObj(beforeDelivery.notes);
	const beforeNotes = asObj(beforeNotesState.notes);
	const beforeLength = Number(asObj(beforeNotes.editor).textLength ?? -1);
	const transcript = ` D02-${RUN_ID}`;
	driver.send({
		type: "pushDictationResult",
		requestId: `${RUN_ID}-d02-push-context-fallback`,
		transcript,
	});
	const deliveredMain = await waitUntil(
		"D02 delivery receipt",
		() => mainState(driver),
		(state) =>
			Number(asObj(asObj(state.dictation).lastDelivery).generation ?? 0) >
			beforeGeneration,
		10_000,
	);
	const deliveryReceipt = asObj(asObj(deliveredMain.dictation).lastDelivery);
	const afterDelivery = await capturePoint(driver, "d02:after-delivery");
	const afterNotesState = asObj(afterDelivery.notes);
	const afterNotes = asObj(afterNotesState.notes);
	const afterLength = Number(asObj(afterNotes.editor).textLength ?? -1);
	const afterMain = asObj(afterDelivery.main);
	const afterWindows = asObj(afterDelivery.windows);
	const targetText = String(
		deliveryReceipt.target ?? deliveryReceipt.targetLabel ?? "",
	).toLowerCase();
	const notesReceived = afterLength === beforeLength + transcript.length;
	check(
		"d02_ui_fallback_targeted_notes",
		targetText.includes("notes"),
		"product",
		deliveryReceipt,
	);
	check("d02_notes_received_transcript", notesReceived, "product", {
		beforeLength,
		afterLength,
		transcriptLength: transcript.length,
	});
	check(
		"d02_main_filter_unchanged",
		afterMain.inputValue === mainMarker,
		"product",
		{
			expected: mainMarker,
			actual: afterMain.inputValue,
		},
	);
	check("d02_captured_at_start_contract_observed", false, "harness", {
		missingPrimitive:
			"set/start context dictation session target without microphone capture",
		observedSubset: "target omitted -> UI fallback resolved at delivery",
	});
	receipt.observations = {
		launcherBefore,
		opened,
		beforeDelivery,
		deliveredMain,
		deliveryReceipt,
		afterDelivery,
		beforeLength,
		afterLength,
	};
	receipt.proposedContract = {
		observedSubset: {
			notesOpenWhileMainFocused: Boolean(
				notesWindow(asObj(beforeDelivery.windows)),
			),
			startFocusedWindowId:
				asObj(beforeDelivery.windows).focusedWindowId ?? null,
			uiFallbackRecipient:
				deliveryReceipt.target ?? deliveryReceipt.targetLabel ?? null,
			notesReceived,
			mainFilterPreserved: afterMain.inputValue === mainMarker,
			finishFocusedWindowId: afterWindows.focusedWindowId ?? null,
		},
		unobserved:
			"captured-at-start target persistence and orchestrator FinishDictation focus",
	};
}

const ROWS: Array<{
	rowId: string;
	run: (
		driver: Driver,
		receipt: RowReceipt,
		check: (name: string, ok: boolean, kind: CheckKind, detail?: Json) => void,
	) => Promise<void>;
}> = [
	{ rowId: "T01-T06", run: t01t06 },
	{ rowId: "T02", run: t02 },
	{ rowId: "T03", run: t03 },
	{ rowId: "T04-T05", run: t04t05 },
	{ rowId: "D02", run: d02 },
];

const summary: Obj = {
	schemaVersion: 1,
	tool: "chaos-28-window-choreography-probe",
	nn: 28,
	runId: RUN_ID,
	binary: BINARY,
	rows: ROWS.map((row) => row.rowId),
	rowClassifications: [],
	productFindings: [],
	harnessFindings: [],
	environmentFindings: [],
};

process.stderr.write(`[driver] binary: ${BINARY} (explicit NN=28 pin)\n`);
let driver: Driver;
try {
	driver = await Driver.launch({
		binary: BINARY,
		sandboxHome: true,
		sessionName: `runner-2-nn28-${RUN_ID}`,
		readyTimeoutMs: 20_000,
		defaultTimeoutMs: 10_000,
		env: {
			SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
			SCRIPT_KIT_BRAIN_TZ: process.env.SCRIPT_KIT_BRAIN_TZ ?? "America/Denver",
		},
	});
} catch (error) {
	const message = String(error);
	summary.classification = /timeout|sandbox|operation not permitted/i.test(
		message,
	)
		? "blocked-by-environment"
		: "invalid-harness";
	summary.launchError = message;
	await writeJson(join(OUTPUT_DIR, "summary.json"), summary);
	process.stderr.write(`${JSON.stringify(summary, null, 2)}\n`);
	process.exit(summary.classification === "blocked-by-environment" ? 3 : 2);
}

const sandboxHome = join(driver.sessionDir, "home");
const dbPath = join(sandboxHome, ".scriptkit", "db", "notes.sqlite");
summary.sandboxHome = sandboxHome;
summary.sessionDir = driver.sessionDir;

async function executeRow(
	rowId: string,
	run: (
		driver: Driver,
		receipt: RowReceipt,
		check: (name: string, ok: boolean, kind: CheckKind, detail?: Json) => void,
	) => Promise<void>,
): Promise<void> {
	const rowDir = join(OUTPUT_DIR, rowId);
	mkdirSync(rowDir, { recursive: true });
	const started = Date.now();
	const receipt: RowReceipt = {
		rowId,
		startedAt: new Date(started).toISOString(),
		classification: "running",
		checks: [],
		observations: {},
		proposedContract: {},
	};
	const beforeLogs = asObj(
		await driver.getLogs({ limit: 2_000 }, { timeoutMs: 8_000 }),
	);
	const baseline = new Set(
		(beforeLogs.entries ?? []).map((entry: Json) => JSON.stringify(entry)),
	);
	const check = (
		name: string,
		ok: boolean,
		kind: CheckKind,
		detail: Json = {},
	) => {
		receipt.checks.push({ name, ok, kind, detail });
		if (ok) return;
		const finding = { rowId, name, detail };
		if (kind === "product") summary.productFindings.push(finding);
		else if (kind === "environment") summary.environmentFindings.push(finding);
		else summary.harnessFindings.push(finding);
	};
	try {
		await run(driver, receipt, check);
	} catch (error) {
		const message =
			error instanceof Error ? (error.stack ?? error.message) : String(error);
		if (error instanceof EnvironmentBlocked) {
			summary.environmentFindings.push({ rowId, message });
		} else {
			summary.harnessFindings.push({ rowId, message });
		}
	}
	const finalPoint = await safe(() =>
		capturePoint(driver, `${rowId}:bundle-final`),
	);
	const [layoutMain, layoutNotes, elementsMain, elementsNotes, windows, logs] =
		await Promise.all([
			safe(() => driver.getLayoutInfo({ target: MAIN }, { timeoutMs: 8_000 })),
			(await notesOpen(driver))
				? safe(() =>
						driver.getLayoutInfo({ target: NOTES }, { timeoutMs: 8_000 }),
					)
				: Promise.resolve(null),
			safe(() =>
				driver.getElements(
					{ target: MAIN, limit: 1_000 },
					{ timeoutMs: 8_000 },
				),
			),
			(await notesOpen(driver))
				? safe(() =>
						driver.getElements(
							{ target: NOTES, limit: 1_000 },
							{ timeoutMs: 8_000 },
						),
					)
				: Promise.resolve(null),
			safe(() => listWindows(driver)),
			safe(() => driver.getLogs({ limit: 2_000 }, { timeoutMs: 8_000 })),
		]);
	const freshErrors = Array.isArray(asObj(logs).entries)
		? asObj(logs).entries.filter(
				(entry: Json) =>
					String(entry.level ?? "").toLowerCase() === "error" &&
					!baseline.has(JSON.stringify(entry)),
			)
		: [];
	check("no_new_error_logs", freshErrors.length === 0, "product", {
		freshErrors,
	});
	const rowProduct = summary.productFindings.some(
		(finding: Obj) => finding.rowId === rowId,
	);
	const rowHarness = summary.harnessFindings.some(
		(finding: Obj) => finding.rowId === rowId,
	);
	const rowEnvironment = summary.environmentFindings.some(
		(finding: Obj) => finding.rowId === rowId,
	);
	receipt.classification = rowEnvironment
		? "blocked-by-environment"
		: rowHarness
			? "invalid-harness"
			: rowProduct
				? "failed-product"
				: "observed-proposed-contract";
	receipt.finishedAt = new Date().toISOString();
	await Promise.all([
		writeJson(join(rowDir, "receipt.json"), receipt),
		writeJson(join(rowDir, "adjudication.json"), {
			rowId,
			classification: receipt.classification,
			proposedContract: receipt.proposedContract,
			checks: receipt.checks,
		}),
		writeJson(join(rowDir, "state-samples.json"), {
			finalPoint,
			observations: receipt.observations,
		}),
		writeJson(join(rowDir, "windows.json"), windows),
		writeJson(join(rowDir, "elements.json"), {
			main: elementsMain,
			notes: elementsNotes,
		}),
		writeJson(join(rowDir, "layout.json"), {
			main: layoutMain,
			notes: layoutNotes,
		}),
		writeJson(join(rowDir, "app-logs.json"), logs),
		writeJson(join(rowDir, "timings.json"), {
			startedAtMs: started,
			finishedAtMs: Date.now(),
			durationMs: Date.now() - started,
		}),
		writeJson(join(rowDir, "database.json"), {
			path: dbPath,
			exists: existsSync(dbPath),
			size: existsSync(dbPath) ? Bun.file(dbPath).size : 0,
		}),
	]);
	summary.rowClassifications.push({
		rowId,
		classification: receipt.classification,
	});
}

try {
	for (const row of ROWS) await executeRow(row.rowId, row.run);
} finally {
	await ensureNotesClosed(driver, "cleanup").catch(() => {});
	driver.send({ type: "hide", requestId: `${RUN_ID}-cleanup-hide` });
	await driver
		.waitForState({ windowVisible: false }, { timeoutMs: 8_000 })
		.catch(() => {});
	const cleanupState = await safe(() => mainState(driver));
	const cleanupWindows = await safe(() => listWindows(driver));
	await driver.close().catch(() => {});
	await Bun.sleep(100);
	if (existsSync(driver.logPath))
		copyFileSync(driver.logPath, join(OUTPUT_DIR, "full-app.log"));
	const latestSession = join(
		sandboxHome,
		".scriptkit",
		"logs",
		"latest-session.jsonl",
	);
	if (existsSync(latestSession))
		copyFileSync(latestSession, join(OUTPUT_DIR, "full-session.jsonl"));
	const protocolResponses = join(
		driver.sessionDir,
		"protocol-responses.ndjson",
	);
	if (existsSync(protocolResponses))
		copyFileSync(
			protocolResponses,
			join(OUTPUT_DIR, "protocol-responses.ndjson"),
		);
	const binaryPattern = `^${BINARY.replace(/[\\^$.*+?()[\]{}|]/g, "\\$&")}([[:space:]]|$)`;
	const processProbe = Bun.spawnSync(["pgrep", "-f", binaryPattern], {
		stdout: "pipe",
		stderr: "pipe",
	});
	summary.cleanup = {
		state: cleanupState,
		windows: cleanupWindows,
		finalization: driver.finalization,
		processCheck: {
			exitCode: processProbe.exitCode,
			stdout: processProbe.stdout.toString().trim(),
			stderr: processProbe.stderr.toString().trim(),
			clean: processProbe.exitCode === 1,
		},
	};
	await writeJson(
		join(OUTPUT_DIR, "post-teardown-process.json"),
		summary.cleanup,
	);
}

summary.classification = summary.environmentFindings.length
	? "blocked-by-environment"
	: summary.harnessFindings.length
		? "invalid-harness"
		: summary.productFindings.length
			? "failed-product"
			: "observed-proposed-contract";
summary.pass = summary.classification === "observed-proposed-contract";
await writeJson(join(OUTPUT_DIR, "summary.json"), summary);
process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
process.exit(
	summary.classification === "observed-proposed-contract"
		? 0
		: summary.classification === "failed-product"
			? 1
			: summary.classification === "invalid-harness"
				? 2
				: 3,
);

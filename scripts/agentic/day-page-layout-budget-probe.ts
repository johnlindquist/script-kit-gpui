#!/usr/bin/env bun

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";
import { openDayPage } from "./day-page-open-helper";

const binary =
	process.env.PROBE_BINARY ??
	"target-agent/artifacts/day-page-layout/script-kit-gpui";
const receiptPath = resolve(
	process.env.PROBE_RECEIPT ?? ".test-output/day-page-layout-budget-probe.json",
);
const runId = `day-page-layout-${Date.now().toString(36)}`;

function localDateStamp(): string {
	const parts = new Intl.DateTimeFormat("en-US", {
		timeZone: "America/Denver",
		year: "numeric",
		month: "2-digit",
		day: "2-digit",
	}).formatToParts(new Date());
	const value = (type: Intl.DateTimeFormatPartTypes) =>
		parts.find((part) => part.type === type)?.value ?? "";
	return `${value("year")}-${value("month")}-${value("day")}`;
}

function component(layout: Json, name: string): Json | null {
	return (
		((layout.components ?? []) as Json[]).find((item) => item.name === name) ??
		null
	);
}

function elementBySource(elements: Json, source: string): Json | null {
	return (
		((elements.elements ?? []) as Json[]).find(
			(item) => item.source === source,
		) ?? null
	);
}

function near(left: unknown, right: unknown, tolerance = 0.01): boolean {
	return Math.abs(Number(left) - Number(right)) <= tolerance;
}

mkdirSync(dirname(receiptPath), { recursive: true });
const receipt: Json = {
	schemaVersion: 1,
	tool: "day-page-layout-budget-probe",
	binary,
	pass: false,
	failures: [],
};

function check(name: string, ok: boolean, detail: Json = {}) {
	if (!ok) receipt.failures.push({ name, ...detail });
}

let driver: Driver | null = null;
try {
	driver = await Driver.launch({
		binary,
		sandboxHome: true,
		sessionName: "day-page-layout-budget",
		readyTimeoutMs: 60_000,
		defaultTimeoutMs: 10_000,
		env: {
			SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
			SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
		},
	});
	const todayPath = join(
		driver.sessionDir,
		"home/.scriptkit/brain/days",
		`${localDateStamp()}.md`,
	);
	mkdirSync(dirname(todayPath), { recursive: true });
	const shelfRows = Array.from(
		{ length: 20 },
		(_, index) =>
			`09:${String(index).padStart(2, "0")} [Clipboard entry](kit://clipboard-history?id=${runId}-${index})`,
	);
	writeFileSync(
		todayPath,
		[`# Layout budget`, "", ...shelfRows, ""].join("\n"),
	);

	receipt.sessionDir = driver.sessionDir;
	receipt.todayPath = todayPath;
	receipt.opened = await openDayPage(driver, runId);

	const collapsedLayout = (await driver.getLayoutInfo(
		{},
		{ timeoutMs: 10_000 },
	)) as Json;
	const collapsedElements = (await driver.getElements(
		{ limit: 80 },
		{ timeoutMs: 10_000 },
	)) as Json;
	const collapsedState = (await driver.getState({ timeoutMs: 10_000 })) as Json;
	const collapsedTextPlane = component(
		collapsedLayout,
		"DayPageEditorTextPlane",
	);
	const collapsedShelf = component(collapsedLayout, "DayPageClipboardShelf");
	const collapsedEditor = component(collapsedLayout, "DayPageEditor");
	const collapsedSurface = component(collapsedLayout, "DayPageSurface");
	const toggle = component(collapsedLayout, "DayPageClipboardShelfToggle");

	receipt.collapsed = {
		state: collapsedState,
		elements: collapsedElements,
		layout: collapsedLayout,
		textPlane: collapsedTextPlane?.bounds ?? null,
		shelf: collapsedShelf?.bounds ?? null,
		editor: collapsedEditor?.bounds ?? null,
	};

	check(
		"day_page_specific_layout_present",
		collapsedTextPlane != null && collapsedShelf != null,
	);
	check(
		"generic_split_layout_absent",
		component(collapsedLayout, "ScriptList") == null &&
			component(collapsedLayout, "PreviewPanel") == null,
	);
	check(
		"collapsed_shelf_shares_editor_text_plane",
		near(collapsedTextPlane?.bounds?.x, collapsedShelf?.bounds?.x) &&
			near(collapsedTextPlane?.bounds?.width, collapsedShelf?.bounds?.width),
		{ textPlane: collapsedTextPlane?.bounds, shelf: collapsedShelf?.bounds },
	);
	check(
		"semantic_text_plane_receipt_present",
		elementBySource(collapsedElements, "DayPageEditorTextPlane") != null,
	);
	check(
		"semantic_shelf_receipt_present",
		elementBySource(collapsedElements, "DayPageClipboardShelf") != null,
	);
	check(
		"native_footer_contract_preserved",
		collapsedState.activeFooter?.expectedSurface === "day_page" &&
			component(collapsedLayout, "MainViewFooter") != null,
	);

	const toggleBounds = toggle?.bounds as Json | undefined;
	if (toggleBounds) {
		receipt.toggleClick = await driver.simulateGpuiClick(
			Number(toggleBounds.x) + 8,
			Number(toggleBounds.y) + Number(toggleBounds.height) / 2,
			{ timeoutMs: 10_000 },
		);
		await new Promise((resolveDelay) => setTimeout(resolveDelay, 250));
	} else {
		check("shelf_toggle_bounds_present", false);
	}

	const expandedLayout = (await driver.getLayoutInfo(
		{},
		{ timeoutMs: 10_000 },
	)) as Json;
	const expandedElements = (await driver.getElements(
		{ limit: 80 },
		{ timeoutMs: 10_000 },
	)) as Json;
	const expandedTextPlane = component(expandedLayout, "DayPageEditorTextPlane");
	const expandedShelf = component(expandedLayout, "DayPageClipboardShelf");
	const expandedList = component(expandedLayout, "DayPageClipboardShelfList");
	const expandedEditor = component(expandedLayout, "DayPageEditor");
	const expandedSurface = component(expandedLayout, "DayPageSurface");
	const semanticShelf = elementBySource(
		expandedElements,
		"DayPageClipboardShelf",
	);

	receipt.expanded = {
		elements: expandedElements,
		layout: expandedLayout,
		textPlane: expandedTextPlane?.bounds ?? null,
		shelf: expandedShelf?.bounds ?? null,
		list: expandedList?.bounds ?? null,
		editor: expandedEditor?.bounds ?? null,
	};

	check("shelf_expanded", semanticShelf?.value === "expanded", {
		semanticShelf,
	});
	check(
		"expanded_shelf_shares_editor_text_plane",
		near(expandedTextPlane?.bounds?.x, expandedShelf?.bounds?.x) &&
			near(expandedTextPlane?.bounds?.width, expandedShelf?.bounds?.width) &&
			near(expandedTextPlane?.bounds?.x, expandedList?.bounds?.x) &&
			near(expandedTextPlane?.bounds?.width, expandedList?.bounds?.width),
		{
			textPlane: expandedTextPlane?.bounds,
			shelf: expandedShelf?.bounds,
			list: expandedList?.bounds,
		},
	);
	check(
		"expanded_list_uses_responsive_cap",
		Number(expandedList?.bounds?.height) > 0 &&
			Number(expandedList?.bounds?.height) < 180,
		{ list: expandedList?.bounds },
	);
	check(
		"expanded_shelf_preserves_editor_minimum",
		Number(expandedEditor?.bounds?.height) >= 180,
		{ editor: expandedEditor?.bounds },
	);
	check(
		"vertical_budget_balances",
		near(
			Number(expandedEditor?.bounds?.height) +
				Number(expandedShelf?.bounds?.height),
			expandedSurface?.bounds?.height,
		),
		{
			editor: expandedEditor?.bounds,
			shelf: expandedShelf?.bounds,
			surface: expandedSurface?.bounds,
		},
	);

	receipt.pass = receipt.failures.length === 0;
} catch (error) {
	receipt.failures.push({
		name: "probe_exception",
		error:
			error instanceof Error ? (error.stack ?? error.message) : String(error),
	});
} finally {
	writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
	await driver?.close();
}

console.log(
	JSON.stringify({
		receiptPath,
		pass: receipt.pass,
		failures: receipt.failures,
	}),
);
if (!receipt.pass) process.exitCode = 1;

#!/usr/bin/env bun
/** Comparable WP1 launcher-wheel receipt. Attaches to an external session. */

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Driver, type Json } from "../devtools/driver.ts";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const target = { type: "main" };
const fixtureButton = "button:dev-style-tool-open-main-window-kitchen-sink";
const requiredScrollFields = [
	"scrollTop",
	"scrollTopItem",
	"scrollTopOffset",
	"firstVisibleIndex",
	"lastVisibleIndexExclusive",
	"firstVisibleSemanticId",
	"lastVisibleSemanticId",
	"selectedIndex",
	"selectedSemanticId",
	"selectedStableKey",
	"selectedRowVisible",
	"selectedRowWithinSafeViewport",
	"hoveredIndex",
	"hoveredSemanticId",
	"hoverSuppressedUntilPointerMove",
	"inputMode",
	"focusedSemanticId",
	"lastInteractionSource",
	"performance",
] as const;

function argValue(name: string): string | null {
	const index = process.argv.indexOf(name);
	return index >= 0 ? (process.argv[index + 1] ?? null) : null;
}

function finite(value: unknown): number | null {
	return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function responseOf(result: Json): Json {
	const response = result.response;
	return response && typeof response === "object" && !Array.isArray(response)
		? (response as Json)
		: result;
}

function scrollOf(state: Json): Json {
	const scroll = state.mainListScroll;
	if (!scroll || typeof scroll !== "object" || Array.isArray(scroll)) {
		throw new Error("getState omitted mainListScroll on ScriptList");
	}
	return scroll as Json;
}

function compactScroll(state: Json): Json {
	const scroll = scrollOf(state);
	return {
		scrollTop: scroll.scrollTop ?? null,
		scrollTopItem: scroll.scrollTopItem ?? null,
		scrollTopOffset: scroll.scrollTopOffset ?? null,
		firstVisibleIndex: scroll.firstVisibleIndex ?? null,
		lastVisibleIndexExclusive: scroll.lastVisibleIndexExclusive ?? null,
		firstVisibleSemanticId: scroll.firstVisibleSemanticId ?? null,
		lastVisibleSemanticId: scroll.lastVisibleSemanticId ?? null,
		selectedIndex: scroll.selectedIndex ?? null,
		selectedSemanticId: scroll.selectedSemanticId ?? null,
		selectedStableKey: scroll.selectedStableKey ?? null,
		selectedRowVisible: scroll.selectedRowVisible ?? null,
		selectedRowWithinSafeViewport: scroll.selectedRowWithinSafeViewport ?? null,
		hoveredIndex: scroll.hoveredIndex ?? null,
		hoveredSemanticId: scroll.hoveredSemanticId ?? null,
		hoverSuppressedUntilPointerMove:
			scroll.hoverSuppressedUntilPointerMove ?? null,
		inputMode: scroll.inputMode ?? null,
		focusedSemanticId: scroll.focusedSemanticId ?? null,
		lastInteractionSource: scroll.lastInteractionSource ?? null,
		performance: scroll.performance ?? null,
	};
}

function compactDispatch(response: Json, index: number, deltaY: number): Json {
	const receipt = responseOf(response);
	return {
		index,
		deltaY,
		success: receipt.success ?? null,
		dispatched: receipt.dispatched ?? null,
		dispatchPath: receipt.dispatchPath ?? null,
		activationProof: receipt.activationProof ?? null,
	};
}

async function waitForWindowKind(
	driver: Awaited<ReturnType<typeof Driver.attach>>,
	kind: string,
	timeoutMs: number,
): Promise<void> {
	const deadline = performance.now() + timeoutMs;
	while (performance.now() < deadline) {
		const receipt = responseOf(
			await driver.listAutomationWindows({ timeoutMs: 5_000 }),
		);
		const windows = Array.isArray(receipt.windows) ? receipt.windows : [];
		if (windows.some((window: Json) => window.kind === kind)) return;
		await new Promise((resolve) => setTimeout(resolve, 40));
	}
	throw new Error(
		`automation window '${kind}' did not appear; start the session with SCRIPT_KIT_STYLE_DEVTOOLS=1`,
	);
}

async function waitForStateWhere(
	driver: Awaited<ReturnType<typeof Driver.attach>>,
	predicate: (state: Json) => boolean,
	timeoutMs: number,
): Promise<Json> {
	const deadline = performance.now() + timeoutMs;
	let last: Json = {};
	while (performance.now() < deadline) {
		last = responseOf(
			await driver.getState({ timeoutMs: Math.min(timeoutMs, 5_000) }),
		);
		if (predicate(last)) return last;
		await new Promise((resolve) => setTimeout(resolve, 20));
	}
	throw new Error(`timed out waiting for probe state: ${JSON.stringify(last)}`);
}

async function main() {
	const session = argValue("--session");
	const mode = argValue("--mode");
	if (!session || mode !== "baseline-red") {
		throw new Error(
			"Usage: bun scripts/agentic/native-list-scroll-probe.ts --session <name> --mode baseline-red [--output <path>]",
		);
	}
	const outputPath = resolve(
		repoRoot,
		argValue("--output") ?? ".test-output/native-list-scroll/baseline-red.json",
	);
	const driver = await Driver.attach({ session, defaultTimeoutMs: 10_000 });
	const checks: Json[] = [];
	const check = (name: string, pass: boolean, observed: Json | null = null) => {
		checks.push({ name, pass, observed });
	};

	try {
		driver.send({ type: "show" });
		await driver.waitForState({ windowVisible: true }, { timeoutMs: 10_000 });
		await waitForWindowKind(driver, "devStyleTool", 10_000);
		const fixtureReceipt = await driver.request(
			{
				type: "batch",
				target: { type: "kind", kind: "devStyleTool" },
				commands: [
					{
						type: "selectBySemanticId",
						semanticId: fixtureButton,
						submit: true,
					},
				],
				options: { stopOnError: true, timeout: 8_000 },
			},
			{ expect: "batchResult", timeoutMs: 10_000 },
		);
		const before = await waitForStateWhere(
			driver,
			(state) => {
				const scroll = state.mainListScroll as Json | undefined;
				return (
					state.surfaceContract?.surfaceKind === "ScriptList" &&
					finite(scroll?.itemCount) !== null &&
					Number(scroll?.itemCount) >= 20 &&
					Number(scroll?.maxScrollTop) > 0
				);
			},
			10_000,
		);

		const layout = responseOf(
			await driver.getLayoutInfo({ target }, { timeoutMs: 8_000 }),
		);
		const scriptList = (
			Array.isArray(layout.components) ? layout.components : []
		).find((component: Json) => component.name === "ScriptList") as
			| Json
			| undefined;
		const bounds = scriptList?.bounds as Json | undefined;
		if (
			!bounds ||
			finite(bounds.x) === null ||
			finite(bounds.y) === null ||
			finite(bounds.width) === null ||
			finite(bounds.height) === null
		) {
			throw new Error(
				`getLayoutInfo omitted ScriptList bounds: ${JSON.stringify(scriptList)}`,
			);
		}
		const point = {
			x: Number(bounds.x) + Number(bounds.width) / 2,
			y: Number(bounds.y) + Number(bounds.height) / 2,
		};

		const dispatchReceipts: Json[] = [];
		const samples: Json[] = [compactScroll(before)];
		const began = await driver.simulateGpuiScrollWheel(
			{
				...point,
				deltaX: 0,
				deltaY: 0,
				phase: "started",
				directPhase: "began",
				momentumPhase: "none",
				timestampSeconds: 1,
			},
			{ target },
		);
		dispatchReceipts.push(compactDispatch(began, 0, 0));

		for (let index = 1; index <= 36; index += 1) {
			const response = await driver.simulateGpuiScrollWheel(
				{
					...point,
					deltaX: 0,
					deltaY: -11,
					phase: "moved",
					directPhase: "changed",
					momentumPhase: "none",
					timestampSeconds: 1 + index / 120,
				},
				{ target },
			);
			dispatchReceipts.push(compactDispatch(response, index, -11));
			await new Promise((resolve) => setTimeout(resolve, 18));
			samples.push(compactScroll(responseOf(await driver.getState())));
		}
		const ended = await driver.simulateGpuiScrollWheel(
			{
				...point,
				deltaX: 0,
				deltaY: 0,
				phase: "ended",
				directPhase: "ended",
				momentumPhase: "none",
				timestampSeconds: 1.5,
			},
			{ target },
		);
		dispatchReceipts.push(compactDispatch(ended, 37, 0));
		await new Promise((resolve) => setTimeout(resolve, 34));
		const after = responseOf(await driver.getState());
		samples.push(compactScroll(after));

		const beforeScroll = scrollOf(before);
		const afterScroll = scrollOf(after);
		const changedTopSamples = samples.filter(
			(sample, index) =>
				index > 0 &&
				Number(sample.scrollTop) !== Number(samples[index - 1].scrollTop),
		);
		const changedScrollTops = [
			Number(samples[0]?.scrollTop ?? 0),
			...changedTopSamples.map((sample) => Number(sample.scrollTop)),
		];
		const changedScrollTopDeltas = changedScrollTops
			.slice(1)
			.map((value, index) => Math.abs(value - changedScrollTops[index]));
		const repeatedRowDeltas = changedScrollTopDeltas.slice(1);
		const rowSteppedScrollTopSequence =
			repeatedRowDeltas.length >= 2 &&
			repeatedRowDeltas.every((delta) => delta >= 32 && delta <= 64) &&
			Math.max(...repeatedRowDeltas) - Math.min(...repeatedRowDeltas) <= 4;
		const wheelChangedSelection =
			Number(afterScroll.selectedIndex) !== Number(beforeScroll.selectedIndex);
		const wheelChangedViewport =
			Number(afterScroll.scrollTop) !== Number(beforeScroll.scrollTop);
		const focusedSemanticIdStable =
			typeof beforeScroll.focusedSemanticId === "string" &&
			beforeScroll.focusedSemanticId.length > 0 &&
			beforeScroll.focusedSemanticId === afterScroll.focusedSemanticId;
		const performance = afterScroll.performance as Json | undefined;
		const performancePopulated =
			performance?.enabled === true &&
			Number(performance.eventCount) >= 38 &&
			Number(performance.frameCallbackCount) > 0 &&
			finite(performance.eventToFrameMsP95) !== null &&
			finite(performance.frameIntervalMsP95) !== null;
		const missingSchemaFields = requiredScrollFields.filter(
			(field) => !Object.hasOwn(afterScroll, field),
		);

		check(
			"deterministic-fixture-opened",
			responseOf(fixtureReceipt).success === true,
			{
				semanticId: fixtureButton,
				batchSuccess: responseOf(fixtureReceipt).success ?? null,
			},
		);
		check(
			"main-list-scroll-schema-complete",
			missingSchemaFields.length === 0,
			{
				missingSchemaFields,
			},
		);
		check("wheel-changed-selection-red", wheelChangedSelection, {
			before: beforeScroll.selectedIndex,
			after: afterScroll.selectedIndex,
		});
		check("wheel-changed-viewport-red", wheelChangedViewport, {
			before: beforeScroll.scrollTop,
			after: afterScroll.scrollTop,
		});
		check("row-stepped-scroll-top-sequence-red", rowSteppedScrollTopSequence, {
			changedSampleCount: changedTopSamples.length,
			changedScrollTops,
			changedScrollTopDeltas,
		});
		check("focused-semantic-id-stable", focusedSemanticIdStable, {
			before: beforeScroll.focusedSemanticId,
			after: afterScroll.focusedSemanticId,
		});
		check(
			"visible-range-populated",
			Number(afterScroll.lastVisibleIndexExclusive) >
				Number(afterScroll.firstVisibleIndex),
			{
				first: afterScroll.firstVisibleIndex,
				lastExclusive: afterScroll.lastVisibleIndexExclusive,
			},
		);
		check(
			"frame-callback-proxy-metrics-populated",
			performancePopulated,
			performance ?? null,
		);
		check(
			"every-gpui-dispatch-succeeded",
			dispatchReceipts.every((receipt) => receipt.success === true),
			{ count: dispatchReceipts.length },
		);

		const passed = checks.every((entry) => entry.pass === true);
		const receipt: Json = {
			schemaVersion: 1,
			tool: "native-list-scroll-probe",
			mode,
			classification: passed ? "reproduced" : "not-reproduced",
			session,
			fixture: {
				semanticId: fixtureButton,
				batchSuccess: responseOf(fixtureReceipt).success ?? null,
				itemCount: beforeScroll.itemCount ?? null,
			},
			eventSequence: {
				coordinate: point,
				changedEventCount: 36,
				changedDeltaY: -11,
				directPhases: ["began", "changed", "ended"],
				dispatchReceipts,
			},
			contract: {
				wheelChangedSelection,
				wheelChangedViewport,
				rowSteppedScrollTopSequence,
				focusedSemanticIdStable,
			},
			thresholdsForGreen: {
				framesOver33_3Ms: Number(performance?.framesOver33_3Ms ?? 0),
				frameIntervalMsP95Max: Math.max(
					16.7,
					Number(performance?.frameIntervalMsP95 ?? 0) * 1.05,
				),
				eventToFrameMsP95Max: Math.max(
					20,
					Number(performance?.eventToFrameMsP95 ?? 0) * 1.05,
				),
			},
			before: compactScroll(before),
			samples,
			after: compactScroll(after),
			checks,
		};
		mkdirSync(dirname(outputPath), { recursive: true });
		writeFileSync(outputPath, `${JSON.stringify(receipt, null, 2)}\n`);
		process.stdout.write(`${JSON.stringify({ ...receipt, outputPath })}\n`);
		if (!passed) process.exitCode = 1;
	} finally {
		await driver.close();
	}
}

await main();

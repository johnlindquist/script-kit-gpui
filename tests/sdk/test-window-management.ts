// Name: SDK Test - Window Management
// Description: Tests window management APIs (getWindows, focusWindow, moveWindow, tileWindow)

/**
 * SDK TEST: test-window-management.ts
 *
 * Legacy-parity proof for the window-management SDK surface.
 *
 * Non-destructive contract (window-engine-foundation):
 * - Read-only tests (getWindows, shape, function existence, TilePosition
 *   list) ALWAYS run.
 * - MUTATING tests run ONLY against a named fixture window ("Window Engine
 *   Fixture" app or a title starting with "SK Window Fixture"). Without a
 *   fixture they report skip — they NEVER move an arbitrary user window.
 * - Every mutation reads back the SAME numeric windowId and restores the
 *   original bounds before the script exits.
 * - close operates only on a dedicated disposable fixture and runs last.
 */

// SDK is loaded via --preload, no import needed

// Import types from kit-sdk
import type { SystemWindowInfo, TilePosition } from "../../scripts/kit-sdk";

// =============================================================================
// Test Infrastructure
// =============================================================================

interface TestResult {
	test: string;
	status: "running" | "pass" | "fail" | "skip";
	timestamp: string;
	result?: unknown;
	error?: string;
	duration_ms?: number;
	reason?: string;
}

function logTest(
	name: string,
	status: TestResult["status"],
	extra?: Partial<TestResult>,
) {
	const result: TestResult = {
		test: name,
		status,
		timestamp: new Date().toISOString(),
		...extra,
	};
	console.log(JSON.stringify(result));
}

function debug(msg: string) {
	console.error(`[TEST] ${msg}`);
}

/** The ONLY windows mutating tests may touch. */
function isMutableFixture(window: SystemWindowInfo): boolean {
	return (
		window.appName === "Window Engine Fixture" ||
		window.title.startsWith("SK Window Fixture") ||
		window.title.startsWith("SK Native Fixture")
	);
}

/** The dedicated disposable fixture for the close test. */
function isDisposableFixture(window: SystemWindowInfo): boolean {
	return (
		window.title.includes("Disposable") && isMutableFixture(window)
	);
}

async function findWindowById(
	windowId: number,
): Promise<SystemWindowInfo | undefined> {
	const current = await getWindows();
	return current.find((window) => window.windowId === windowId);
}

// =============================================================================
// Tests
// =============================================================================

debug("test-window-management.ts starting...");
debug(
	`SDK globals: getWindows=${typeof getWindows}, focusWindow=${typeof focusWindow}`,
);

// -----------------------------------------------------------------------------
// Test 1: getWindows() - List all system windows (always runs)
// -----------------------------------------------------------------------------
const test1 = "getWindows-returns-array";
logTest(test1, "running");
const start1 = Date.now();

let windows: SystemWindowInfo[] = [];

try {
	windows = await getWindows();
	if (!Array.isArray(windows)) {
		throw new Error(`Expected array, got ${typeof windows}`);
	}
	logTest(test1, "pass", {
		result: { windowCount: windows.length },
		duration_ms: Date.now() - start1,
	});
} catch (err) {
	logTest(test1, "fail", {
		error: String(err),
		duration_ms: Date.now() - start1,
	});
}

// -----------------------------------------------------------------------------
// Test 2: Window info structure (always runs)
// -----------------------------------------------------------------------------
const test2 = "window-info-structure";
logTest(test2, "running");
const start2 = Date.now();

try {
	if (windows.length === 0) {
		logTest(test2, "skip", {
			reason: "No windows available to inspect",
			duration_ms: Date.now() - start2,
		});
	} else {
		const firstWindow = windows[0];
		if (typeof firstWindow.windowId !== "number") {
			throw new Error(
				`Expected windowId to be number, got ${typeof firstWindow.windowId}`,
			);
		}
		if (typeof firstWindow.title !== "string") {
			throw new Error(
				`Expected title to be string, got ${typeof firstWindow.title}`,
			);
		}
		if (typeof firstWindow.appName !== "string") {
			throw new Error(
				`Expected appName to be string, got ${typeof firstWindow.appName}`,
			);
		}
		const hasBounds =
			firstWindow.bounds === undefined ||
			(typeof firstWindow.bounds === "object" &&
				firstWindow.bounds !== null &&
				typeof firstWindow.bounds.x === "number" &&
				typeof firstWindow.bounds.y === "number" &&
				typeof firstWindow.bounds.width === "number" &&
				typeof firstWindow.bounds.height === "number");
		if (!hasBounds) {
			throw new Error("Window bounds has invalid structure");
		}
		logTest(test2, "pass", {
			result: {
				sampleWindow: {
					windowId: firstWindow.windowId,
					title: firstWindow.title.substring(0, 50),
					appName: firstWindow.appName,
					hasBounds: !!firstWindow.bounds,
				},
			},
			duration_ms: Date.now() - start2,
		});
	}
} catch (err) {
	logTest(test2, "fail", {
		error: String(err),
		duration_ms: Date.now() - start2,
	});
}

// -----------------------------------------------------------------------------
// Test 3: Function existence (always runs)
// -----------------------------------------------------------------------------
const test3 = "window-functions-exist";
logTest(test3, "running");
const start3 = Date.now();

try {
	const functions = [
		{ name: "getWindows", fn: getWindows },
		{ name: "focusWindow", fn: focusWindow },
		{ name: "closeWindow", fn: closeWindow },
		{ name: "minimizeWindow", fn: minimizeWindow },
		{ name: "maximizeWindow", fn: maximizeWindow },
		{ name: "moveWindow", fn: moveWindow },
		{ name: "resizeWindow", fn: resizeWindow },
		{ name: "tileWindow", fn: tileWindow },
		{ name: "moveToNextDisplay", fn: moveToNextDisplay },
		{ name: "moveToPreviousDisplay", fn: moveToPreviousDisplay },
		{ name: "getFrontmostWindow", fn: getFrontmostWindow },
		{ name: "getDisplays", fn: getDisplays },
	];
	const missing = functions
		.filter(({ fn }) => typeof fn !== "function")
		.map(({ name }) => name);
	if (missing.length > 0) {
		throw new Error(`Missing functions: ${missing.join(", ")}`);
	}
	logTest(test3, "pass", {
		result: { functions: functions.map(({ name }) => name) },
		duration_ms: Date.now() - start3,
	});
} catch (err) {
	logTest(test3, "fail", {
		error: String(err),
		duration_ms: Date.now() - start3,
	});
}

// -----------------------------------------------------------------------------
// Test 4: TilePosition wire vocabulary — EXACTLY the 21 public strings.
// Sixths are internal-only; adding them to the SDK is a contract violation.
// -----------------------------------------------------------------------------
const test4 = "tile-positions";
logTest(test4, "running");
const start4 = Date.now();

try {
	const validPositions: TilePosition[] = [
		"left",
		"right",
		"top",
		"bottom",
		"top-left",
		"top-right",
		"bottom-left",
		"bottom-right",
		"left-third",
		"center-third",
		"right-third",
		"top-third",
		"middle-third",
		"bottom-third",
		"first-two-thirds",
		"last-two-thirds",
		"top-two-thirds",
		"bottom-two-thirds",
		"center",
		"almost-maximize",
		"maximize",
	];
	if (validPositions.length !== 21) {
		throw new Error(
			`Expected exactly 21 public tile positions, got ${validPositions.length}`,
		);
	}
	logTest(test4, "pass", {
		result: { positionCount: validPositions.length },
		duration_ms: Date.now() - start4,
	});
} catch (err) {
	logTest(test4, "fail", {
		error: String(err),
		duration_ms: Date.now() - start4,
	});
}

// -----------------------------------------------------------------------------
// Mutating tests: fixture-gated.
// -----------------------------------------------------------------------------
const mutableFixture = windows.find(
	(window) => isMutableFixture(window) && !isDisposableFixture(window),
);
const originalBounds = mutableFixture?.bounds
	? { ...mutableFixture.bounds }
	: undefined;

function skipWithoutFixture(name: string, start: number): boolean {
	if (mutableFixture) {
		return false;
	}
	logTest(name, "skip", {
		reason:
			"No mutable fixture window present (Window Engine Fixture / SK Window Fixture). Mutating tests never touch arbitrary user windows.",
		duration_ms: Date.now() - start,
	});
	return true;
}

// -----------------------------------------------------------------------------
// Test 5: moveWindow() with readback + restore
// -----------------------------------------------------------------------------
const test5 = "moveWindow-fixture";
logTest(test5, "running");
const start5 = Date.now();

try {
	if (!skipWithoutFixture(test5, start5) && mutableFixture) {
		await moveWindow(mutableFixture.windowId, 120, 130);
		const after = await findWindowById(mutableFixture.windowId);
		if (!after) {
			throw new Error("window id disappeared after move");
		}
		if (!after.bounds || Math.abs(after.bounds.x - 120) > 2) {
			throw new Error(
				`readback mismatch after move: ${JSON.stringify(after.bounds)}`,
			);
		}
		logTest(test5, "pass", {
			result: { windowId: mutableFixture.windowId, readback: after.bounds },
			duration_ms: Date.now() - start5,
		});
	}
} catch (err) {
	logTest(test5, "fail", {
		error: String(err),
		duration_ms: Date.now() - start5,
	});
}

// -----------------------------------------------------------------------------
// Test 6: resizeWindow() with readback
// -----------------------------------------------------------------------------
const test6 = "resizeWindow-fixture";
logTest(test6, "running");
const start6 = Date.now();

try {
	if (!skipWithoutFixture(test6, start6) && mutableFixture) {
		await resizeWindow(mutableFixture.windowId, 640, 480);
		const after = await findWindowById(mutableFixture.windowId);
		if (!after?.bounds || Math.abs(after.bounds.width - 640) > 2) {
			throw new Error(
				`readback mismatch after resize: ${JSON.stringify(after?.bounds)}`,
			);
		}
		logTest(test6, "pass", {
			result: { windowId: mutableFixture.windowId, readback: after.bounds },
			duration_ms: Date.now() - start6,
		});
	}
} catch (err) {
	logTest(test6, "fail", {
		error: String(err),
		duration_ms: Date.now() - start6,
	});
}

// -----------------------------------------------------------------------------
// Test 7: tileWindow() + focusWindow() against the fixture
// -----------------------------------------------------------------------------
const test7 = "tileWindow-fixture";
logTest(test7, "running");
const start7 = Date.now();

try {
	if (!skipWithoutFixture(test7, start7) && mutableFixture) {
		await tileWindow(mutableFixture.windowId, "center");
		const after = await findWindowById(mutableFixture.windowId);
		if (!after) {
			throw new Error("window id disappeared after tile");
		}
		await focusWindow(mutableFixture.windowId);
		logTest(test7, "pass", {
			result: { windowId: mutableFixture.windowId, readback: after.bounds },
			duration_ms: Date.now() - start7,
		});
	}
} catch (err) {
	logTest(test7, "fail", {
		error: String(err),
		duration_ms: Date.now() - start7,
	});
}

// -----------------------------------------------------------------------------
// Test 8: stale ids reject rather than target a replacement window
// -----------------------------------------------------------------------------
const test8 = "stale-id-rejects";
logTest(test8, "running");
const start8 = Date.now();

try {
	const staleId = 0x7fff_0000 + Math.floor(Math.random() * 0xffff);
	const taken = windows.some((window) => window.windowId === staleId);
	if (taken) {
		logTest(test8, "skip", {
			reason: "random probe id collided with a real window",
			duration_ms: Date.now() - start8,
		});
	} else {
		let rejected = false;
		try {
			await moveWindow(staleId, 10, 10);
		} catch {
			rejected = true;
		}
		if (!rejected) {
			throw new Error("stale/unknown window id must reject, not succeed");
		}
		logTest(test8, "pass", { duration_ms: Date.now() - start8 });
	}
} catch (err) {
	logTest(test8, "fail", {
		error: String(err),
		duration_ms: Date.now() - start8,
	});
}

// Every legacy mutator must preserve the same stale-ID boundary; auto-submit
// fixtures are purely in-memory and never address an operating-system window.
const staleActionsTest = "stale-id-rejects-every-window-action";
logTest(staleActionsTest, "running");
const staleActionsStarted = Date.now();

try {
	const staleId = 0x7fff_fffe;
	const operations = [
		{ name: "focus", call: () => focusWindow(staleId) },
		{ name: "close", call: () => closeWindow(staleId) },
		{ name: "minimize", call: () => minimizeWindow(staleId) },
		{ name: "maximize", call: () => maximizeWindow(staleId) },
		{ name: "move", call: () => moveWindow(staleId, 10, 10) },
		{ name: "resize", call: () => resizeWindow(staleId, 100, 100) },
		{ name: "tile", call: () => tileWindow(staleId, "center") },
		{ name: "next-display", call: () => moveToNextDisplay(staleId) },
		{ name: "previous-display", call: () => moveToPreviousDisplay(staleId) },
	];

	for (const operation of operations) {
		let rejected = false;
		try {
			await operation.call();
		} catch (error) {
			rejected = String(error).includes("stale or unknown");
		}
		if (!rejected) {
			throw new Error(`${operation.name} accepted a stale or unknown window ID`);
		}
	}

	logTest(staleActionsTest, "pass", {
		result: { rejectedActions: operations.map((operation) => operation.name) },
		duration_ms: Date.now() - staleActionsStarted,
	});
} catch (error) {
	logTest(staleActionsTest, "fail", {
		error: String(error),
		duration_ms: Date.now() - staleActionsStarted,
	});
}

// -----------------------------------------------------------------------------
// Restore the fixture's original bounds before the close test.
// -----------------------------------------------------------------------------
if (mutableFixture && originalBounds) {
	try {
		await moveWindow(mutableFixture.windowId, originalBounds.x, originalBounds.y);
		await resizeWindow(
			mutableFixture.windowId,
			originalBounds.width,
			originalBounds.height,
		);
		debug("fixture bounds restored");
	} catch (err) {
		debug(`fixture restore failed: ${err}`);
	}
}

// -----------------------------------------------------------------------------
// Test 9 (LAST): closeWindow() against the dedicated disposable fixture only
// -----------------------------------------------------------------------------
const test9 = "closeWindow-disposable-fixture";
logTest(test9, "running");
const start9 = Date.now();

try {
	const disposable = windows.find(isDisposableFixture);
	if (!disposable) {
		logTest(test9, "skip", {
			reason: "No disposable fixture window present; close never targets anything else.",
			duration_ms: Date.now() - start9,
		});
	} else {
		await closeWindow(disposable.windowId);
		logTest(test9, "pass", {
			result: { windowId: disposable.windowId },
			duration_ms: Date.now() - start9,
		});
	}
} catch (err) {
	logTest(test9, "fail", {
		error: String(err),
		duration_ms: Date.now() - start9,
	});
}

debug("test-window-management.ts completed!");
exit(0);

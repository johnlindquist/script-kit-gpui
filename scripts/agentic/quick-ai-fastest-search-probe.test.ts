import { describe, expect, test } from "bun:test";

import { median, traceSummary } from "./quick-ai-fastest-search-probe.ts";

describe("Quick AI fastest-search receipt helpers", () => {
	test("median uses the middle value or averages the two middle values", () => {
		expect(median([27_130, 11_499, 16_571])).toBe(16_571);
		expect(median([18_510, 33_954])).toBe(26_232);
		expect(median([])).toBeNull();
	});

	test("same-item lifecycle counts as one native action and one permit", () => {
		const summary = traceSummary([
			{ event: "spawned", runId: "run", startTurnToSpawnMs: 7 },
			{ event: "search_permit_reserved", elapsedMs: 10 },
			{
				event: "native_web_action",
				nativeLifecyclePhase: "started",
				actionClass: "search",
				actionOrdinal: 1,
			},
			{
				event: "native_web_action",
				nativeLifecyclePhase: "updated",
				actionClass: "search",
				actionOrdinal: 1,
			},
			{
				event: "native_web_action",
				nativeLifecyclePhase: "completed",
				actionClass: "search",
				actionOrdinal: 1,
			},
			{ event: "search_completed", elapsedMs: 40 },
			{
				event: "teardown",
				childReaped: true,
				processGroupAlive: false,
			},
			{ event: "terminal", kind: "completed", elapsedMs: 50 },
		]);
		expect(summary.logicalSearchPermitCount).toBe(1);
		expect(summary.distinctNativeWebActionCount).toBe(1);
		expect(summary.nativeLifecycleEventCount).toBe(3);
		expect(summary.pageFollowActionCount).toBe(0);
		expect(summary.excessWebActionCount).toBe(0);
		expect(summary.searchCompleted).toBe(true);
	});

	test("second item and page follow remain observable invalidating evidence", () => {
		const summary = traceSummary([
			{ event: "spawned", runId: "run" },
			{ event: "search_permit_reserved" },
			{
				event: "native_web_action",
				actionClass: "search",
				actionOrdinal: 1,
			},
			{
				event: "native_web_action",
				actionClass: "page-follow",
				actionOrdinal: 2,
			},
			{ event: "excess_web_action_observed" },
			{
				event: "teardown",
				childReaped: true,
				processGroupAlive: false,
			},
			{
				event: "terminal",
				kind: "recovery",
				failureCode: "QuickAiSearchBudgetExceeded",
			},
		]);
		expect(summary.distinctNativeWebActionCount).toBe(2);
		expect(summary.pageFollowActionCount).toBe(1);
		expect(summary.excessWebActionCount).toBe(1);
		expect(summary.terminal[0].kind).toBe("recovery");
	});

	test("raw provider and tool identifiers are rejected from trace evidence", () => {
		const summary = traceSummary([
			{ event: "spawned", runId: "run" },
			{ event: "native_web_action", actionOrdinal: 1, itemId: "raw" },
			{ event: "terminal", kind: "completed" },
		]);
		expect(summary.rawProviderIdentifierPresent).toBe(true);

		const rawTool = traceSummary([
			{ event: "spawned", runId: "run", toolName: "web_search" },
			{ event: "terminal", kind: "completed" },
		]);
		expect(rawTool.rawToolIdentifierPresent).toBe(true);
	});
});

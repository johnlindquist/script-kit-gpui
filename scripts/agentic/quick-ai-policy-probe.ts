#!/usr/bin/env bun
/**
 * Quick AI search-budget and recovery runtime proof.
 *
 * The AUTHORITATIVE admission logic (classify → adjudicate, tool-policy,
 * forbidden-tool fail-closed, permission rejection, and the zero-context
 * backstop) is locked by the unit matrix in
 * `src/ai/agent_chat/ui/{capabilities,thread}.rs`. This probe confirms, against
 * the REAL binary, that the budget boundary holds at the surfaces a user can
 * reach:
 *
 *   1. Quick AI launches the native, app-budgeted public-web adapter. Either
 *      the Codex exec path runs (`quick_ai_codex_view_switched`; that adapter
 *      enforces one provider item plus one normalized query and rejects any
 *      turn that is not a single user-text block), or the Pi process carries
 *      `--tools web_search` on its real argv.
 *   2. The pre-thread zero-context launch invariant is NOT violated
 *      (`quick_ai_zero_context_launch_invariant_violated` never logged) — the
 *      launch really was clean.
 *   3. A second focused search stops at the typed policy boundary, preserves
 *      real partial output, hides raw protocol details, and offers a primary
 *      Continue in Agent Chat recovery.
 *   4. Continue in Agent Chat opens a fresh standard chat with the original
 *      question and safe source URLs, never an internal failure identifier.
 *
 * NOT automatable through the driver (reported, never faked): drag-and-drop
 * file attach and clipboard image paste.
 *
 * Run: bun scripts/agentic/quick-ai-policy-probe.ts
 *        [--fixture budget|deadline] [--receipt <path>]
 */
import { Driver } from "../devtools/driver.ts";
import {
	chmodSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const binary =
	process.env.SCRIPT_KIT_GPUI_BINARY ??
	"target-agent/artifacts/w2-quickai/script-kit-gpui";

const receiptArgIdx = process.argv.indexOf("--receipt");
const receiptPath =
	receiptArgIdx >= 0 ? process.argv[receiptArgIdx + 1] : undefined;
const fixtureArgIdx = process.argv.indexOf("--fixture");
const fixture = fixtureArgIdx >= 0 ? process.argv[fixtureArgIdx + 1] : "budget";
if (fixture !== "budget" && fixture !== "deadline") {
	throw new Error(`unknown --fixture ${fixture}; expected budget or deadline`);
}
const expectedFailureCode =
	fixture === "budget"
		? "QuickAiSearchBudgetExceeded"
		: "QuickAiDeadlineExceeded";
const expectedTraceEvent =
	fixture === "budget" ? "policy_recovery" : "deadline_expired";

const receipt: Record<string, unknown> = {
	probe: "quick-ai-policy",
	fixture,
	binary,
	authoritativeProofLocation:
		"unit matrix: capabilities::quick_ai_context_admission_matrix + " +
		"thread::tests::quick_ai_* (context ingress, forbidden tool, tool policy)",
	unautomatable: [
		"drag-and-drop file attach (no driver primitive)",
		"clipboard image paste (no driver primitive)",
	],
};

const fixtureDir = mkdtempSync(join(tmpdir(), "quick-ai-policy-"));
const fakeCodex = join(fixtureDir, "codex");
const tracePath = join(fixtureDir, "trace.ndjson");
const terminalFixture =
	fixture === "budget"
		? `printf '%s\\n' '{"type":"item.started","item":{"id":"denied","type":"web_search","action":{"type":"search","query":"second search"}}}'`
		: "sleep 60";
writeFileSync(
	fakeCodex,
	`#!/bin/sh
set -eu
printf '%s\\n' '{"type":"thread.started","thread_id":"t"}'
printf '%s\\n' '{"type":"turn.started"}'
printf '%s\\n' '{"type":"item.started","item":{"id":"first","type":"web_search","action":{"type":"search","query":"capital of france"}}}'
printf '%s\\n' '{"type":"item.completed","item":{"id":"first","type":"web_search","action":{"type":"search","query":"capital of france"}}}'
printf '%s\\n' '{"type":"item.completed","item":{"id":"partial","type":"agent_message","text":"Paris is the capital of France. https://example.com/france"}}'
${terminalFixture}
sleep 60
`,
);
chmodSync(fakeCodex, 0o700);
mkdirSync(resolve(".test-screenshots/ai-rock-solid-ux"), {
	recursive: true,
});

const driver = await Driver.launch({
	sessionName: `quick-ai-policy-${fixture}-probe`,
	binary,
	sandboxHome: true,
	seedAgentAuth: true,
	env: {
		SCRIPT_KIT_CODEX_BIN: fakeCodex,
		SCRIPT_KIT_QUICK_AI_TRACE_PATH: tracePath,
	},
});

/** True if `needle` appears anywhere in the recent log blob. */
async function logSeen(needle: string, timeoutMs = 500): Promise<boolean> {
	const start = performance.now();
	do {
		const logs = await driver.getLogs({ limit: 800 });
		if (JSON.stringify(logs).includes(needle)) return true;
		if (performance.now() - start >= timeoutMs) return false;
		await Bun.sleep(150);
	} while (performance.now() - start < timeoutMs);
	return false;
}

try {
	await driver.waitForSettle();

	// --- Enter Quick AI: text + Tab -----------------------------------------
	await driver.setFilterAndWait("what is the capital of france");
	driver.simulateKey("tab");
	const quickAiEntry = await logSeen("quick_ai_tab_entry", 6000);
	const codexViewSwitched = await logSeen("quick_ai_codex_view_switched", 8000);
	await driver.waitForSettle({ timeoutMs: 8000 });

	driver.send({ type: "show" });
	const recoveryDeadline = performance.now() + 30_000;
	let recoveryState: any = null;
	while (performance.now() < recoveryDeadline) {
		try {
			recoveryState = await driver.request(
				{ type: "getAgentChatState", target: { type: "id", id: "main" } },
				{ expect: "agent_chatStateResult", timeoutMs: 2500 },
			);
		} catch {
			await Bun.sleep(100);
			continue;
		}
		if (recoveryState?.reliability?.phase === "awaitingRecovery") break;
		await Bun.sleep(100);
	}
	receipt.recoveryState = recoveryState;
	await Bun.sleep(300);
	receipt.screenshot = await driver.captureScreenshot({
		target: { type: "id", id: "main" },
		savePath: resolve(
			`.test-screenshots/ai-rock-solid-ux/quick-ai-policy-${fixture}.png`,
		),
		timeoutMs: 10_000,
	});

	// --- Proof 1: web-search-only backend ------------------------------------
	let piLine = "";
	const deadline = performance.now() + 12000;
	while (performance.now() < deadline) {
		const ps = Bun.spawnSync(["pgrep", "-fl", "mode rpc"]);
		const line = ps.stdout
			.toString()
			.split("\n")
			.find(
				(l) =>
					l.includes("gpt-5.3-codex-spark") && l.includes("You are Quick AI"),
			);
		if (line) {
			piLine = line;
			break;
		}
		if (codexViewSwitched) break; // codex exec path — no pi rpc process
		await Bun.sleep(250);
	}
	receipt.launch = { quickAiEntry, codexViewSwitched };
	receipt.tools = {
		codexExecPathUsed: codexViewSwitched,
		piProcessFound: piLine.length > 0,
		piHasWebSearchTool: piLine.includes("--tools web_search"),
		// Either backend is web-search-only: codex exec hardcodes the allowlist and
		// rejects multi-block turns; pi bakes `--tools web_search` onto its argv.
		webSearchOnly:
			codexViewSwitched ||
			(piLine.length > 0 && piLine.includes("--tools web_search")),
	};

	// --- Proof 2: launch invariant was NOT violated --------------------------
	receipt.launchInvariantViolated = await logSeen(
		"quick_ai_zero_context_launch_invariant_violated",
		300,
	);

	const trace = readFileSync(tracePath, "utf8")
		.trim()
		.split("\n")
		.filter(Boolean)
		.map((line) => JSON.parse(line));
	const reliability = recoveryState?.reliability ?? {};
	const recoveryActions = Array.isArray(reliability.recoveryActions)
		? reliability.recoveryActions
		: [];
	receipt.searchBudget = {
		surface: reliability.surface,
		phase: reliability.phase,
		failureCode: reliability.failureCode,
		primaryActionId: reliability.primaryActionId,
		recoveryActions,
		partialOutputFingerprint:
			reliability.preservation?.partialOutputFingerprint ?? null,
		rawPrimaryVisible: reliability.diagnostic?.rawPrimaryVisible,
		policyTraceSeen: trace.some(
			(record) =>
				record.event === expectedTraceEvent &&
				(fixture === "deadline" ||
					(record.failureCode === expectedFailureCode &&
						record.searchBudget === 1 &&
						record.completedSearches === 1)),
		),
		protocolFailureAbsent: !trace.some(
			(record) => record.event === "protocol_failure",
		),
	};

	receipt.handoffAction = await driver.triggerAction(
		"ai-recovery-continue-agent-chat",
	);
	const handoffDeadline = performance.now() + 12_000;
	let handoffState: any = null;
	while (performance.now() < handoffDeadline) {
		handoffState = await driver.request(
			{ type: "getAgentChatState", target: { type: "id", id: "main" } },
			{ expect: "agent_chatStateResult", timeoutMs: 8000 },
		);
		if (
			handoffState?.uiVariant === "standard" &&
			String(handoffState?.inputText ?? "").includes("Quick AI handoff")
		) {
			break;
		}
		await Bun.sleep(100);
	}
	receipt.handoffState = handoffState;

	const checks: Array<[string, boolean]> = [
		["launch.quickAiEntry", quickAiEntry],
		["launch.codexViewSwitched", codexViewSwitched],
		["tools.webSearchOnly", Boolean((receipt.tools as any).webSearchOnly)],
		["launchInvariantNotViolated", !receipt.launchInvariantViolated],
		["budget.surface", reliability.surface === "quickAi"],
		["budget.phase", reliability.phase === "awaitingRecovery"],
		["budget.failureCode", reliability.failureCode === expectedFailureCode],
		[
			"budget.primaryAction",
			reliability.primaryActionId === "ai-recovery-continue-agent-chat",
		],
		["budget.noRetry", !recoveryActions.includes("ai-recovery-retry")],
		[
			"budget.partialPreserved",
			typeof reliability.preservation?.partialOutputFingerprint === "string",
		],
		[
			"budget.rawPrimaryHidden",
			reliability.diagnostic?.rawPrimaryVisible === false,
		],
		[
			"budget.typedTrace",
			Boolean((receipt.searchBudget as any).policyTraceSeen) &&
				Boolean((receipt.searchBudget as any).protocolFailureAbsent),
		],
		["handoff.standard", handoffState?.uiVariant === "standard"],
		[
			"handoff.label",
			String(handoffState?.inputText ?? "").includes("Quick AI handoff"),
		],
		[
			"handoff.question",
			String(handoffState?.inputText ?? "").includes(
				"what is the capital of france",
			),
		],
		[
			"handoff.source",
			String(handoffState?.inputText ?? "").includes(
				"https://example.com/france",
			),
		],
		[
			"handoff.noInternalId",
			!JSON.stringify(handoffState).includes(
				"quick_ai_more_than_two_search_queries",
			),
		],
	];
	receipt.pass = checks.every(([, ok]) => ok);
	receipt.failedChecks = checks.filter(([, ok]) => !ok).map(([n]) => n);
} finally {
	await driver.close();
}

const serialized = JSON.stringify(receipt, null, 2);
if (receiptPath) await Bun.write(receiptPath, serialized);
console.log(serialized);
if (!receipt.pass) process.exit(1);

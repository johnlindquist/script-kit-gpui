#!/usr/bin/env bun
/// <reference types="bun-types" />
// Probe: reproduce Agent Chat scroll lag on a SHORT conversation whose
// markdown rows are below the heavy-markdown preview threshold (the case the
// user reported), drive REAL scroll-wheel events over the window, and profile
// the process with `sample` while scrolling. Writes a receipt JSON.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const RECEIPT = resolve(
	process.env.PROBE_RECEIPT ??
		".test-output/agent-chat-short-scroll-probe.json",
);
const SAMPLE_OUT = resolve(
	process.env.PROBE_SAMPLE_OUT ??
		".test-output/agent-chat-short-scroll-sample.txt",
);
const SCROLL_HELPER = process.env.PROBE_SCROLL_HELPER ?? "";
const BINARY =
	process.env.SCRIPT_KIT_GPUI_BINARY ??
	"target-agent/artifacts/agent-chat-scroll-perf/script-kit-gpui";
const SCROLL_SECONDS = Number(process.env.PROBE_SCROLL_SECONDS ?? "6");

// Below is_scroll_heavy() thresholds so rows render FULL markdown like the
// user's screenshot. Override with PROBE_ASSISTANT_MD=<path> for heavier
// just-under-threshold fixtures.
function assistantMarkdown(ix: number): string {
	const path = process.env.PROBE_ASSISTANT_MD;
	if (path && existsSync(path)) return readFileSync(path, "utf8");
	return `The focus source for turn ${ix} is the activity journal with no journaled activity. Its attention count is 1.

Source context and title: Focus review 2026-07-0${(ix % 9) + 1}.

## Current focus

1. google and youtube have the highest accumulated attention in the provided signals (19 each).
2. zoom, store, gemini, eggo-brand and egghead form the tied second attention cluster (18 each).
3. openrouter and github form the secondary attention tier with sustained signal (14, 13).

## Recent activity

No activity-journal entries were provided, so no user actions can be grouped from logs. The attention topics therefore stand alone as raw signal counts without corroborating behavioral evidence from the journals that would normally anchor them.

## Drifting

lapis-fossil-vsbc.here.now, dashboard and eggo-brand.wzrrd.sh received low-signal attention only (2, 1, 1) with no journaled activity. All attention topics lack matching recent-activity evidence in the provided journals, which suggests the capture pipeline was running while the journaling pipeline was not.

**Summary**: attention capture is healthy, activity journaling is silent, and the recommendation is to verify the journal writer before trusting drift classification for turn ${ix}.`;
}

function percentile(values: number[], p: number): number {
	if (values.length === 0) return 0;
	const sorted = [...values].sort((a, b) => a - b);
	const ix = Math.min(sorted.length - 1, Math.round((sorted.length - 1) * p));
	return sorted[ix];
}

function parseRenderTrace(logPath: string): Json {
	const text = existsSync(logPath) ? readFileSync(logPath, "utf8") : "";
	const elapsed: number[] = [];
	let heavyPreviewMax = 0;
	let markdownViewMax = 0;
	for (const line of text.split("\n")) {
		if (!line.includes("event=agent_chat_transcript_render")) continue;
		const elapsedMatch = line.match(/elapsed_ms=([0-9.]+)/);
		const previewMatch = line.match(/heavy_preview_count=(\d+)/);
		const markdownMatch = line.match(/markdown_view_count=(\d+)/);
		if (elapsedMatch) elapsed.push(Number(elapsedMatch[1]));
		if (previewMatch)
			heavyPreviewMax = Math.max(heavyPreviewMax, Number(previewMatch[1]));
		if (markdownMatch)
			markdownViewMax = Math.max(markdownViewMax, Number(markdownMatch[1]));
	}
	return {
		samples: elapsed.length,
		renderAvgMs:
			elapsed.length > 0
				? Number(
						(elapsed.reduce((a, b) => a + b, 0) / elapsed.length).toFixed(3),
					)
				: 0,
		renderP95Ms: Number(percentile(elapsed, 0.95).toFixed(3)),
		renderMaxMs: Number(Math.max(0, ...elapsed).toFixed(3)),
		heavyPreviewCountMax: heavyPreviewMax,
		fullMarkdownViewCountMax: markdownViewMax,
	};
}

mkdirSync(dirname(RECEIPT), { recursive: true });
const receipt: Json = {
	tool: "agent-chat-short-scroll-probe",
	binary: BINARY,
	pass: false,
	failures: [],
};

const driver = await Driver.launch({
	binary: BINARY,
	sandboxHome: true,
	sessionName: "agent-chat-short-scroll",
	defaultTimeoutMs: 10_000,
	env: {
		SCRIPT_KIT_AGENT_CHAT_RENDER_TRACE: "1",
		SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
		...(process.env.PROBE_EXTRA_ENV_KEY
			? {
					[process.env.PROBE_EXTRA_ENV_KEY]:
						process.env.PROBE_EXTRA_ENV_VALUE ?? "",
				}
			: {}),
	},
});

try {
	receipt.target = { pid: driver.pid, logPath: driver.logPath };

	const opened = await driver.request(
		{ type: "openAgentChatKitchenSinkFixture" },
		{ expect: "externalCommandResult", timeoutMs: 10_000 },
	);
	if (opened.ok === false || opened.success === false) {
		receipt.failures.push({ name: "open_fixture_failed", opened });
	}
	await Bun.sleep(1000);

	const fixture = await driver.request(
		{
			type: "setAgentChatTestFixture",
			phase: "idle",
			userText: "Summarize the focus review please.",
			assistantText: assistantMarkdown(1),
			messageCount: 8,
		},
		{ expect: "externalCommandResult", timeoutMs: 15_000 },
	);
	if (fixture.ok === false || fixture.success === false) {
		receipt.failures.push({ name: "set_fixture_failed", fixture });
	}
	await Bun.sleep(500);

	const windows = await driver.listAutomationWindows({ timeoutMs: 10_000 });
	const agentWindow = Array.isArray(windows.windows)
		? (windows.windows.find(
				(win: Json) =>
					win.semanticSurface === "agentChatChat" && win.visible === true,
			) ??
			windows.windows.find(
				(win: Json) => win.semanticSurface === "agentChatChat",
			))
		: null;
	receipt.window = agentWindow;
	if (!agentWindow) {
		receipt.failures.push({ name: "missing_agent_chat_window" });
		throw new Error("missing agent chat window");
	}

	const bounds = agentWindow.bounds ?? agentWindow.frame ?? null;
	const cx = bounds ? bounds.x + bounds.width / 2 : 0;
	const cy = bounds ? bounds.y + bounds.height / 2 : 0;
	receipt.scrollPoint = { x: cx, y: cy };

	// Start CPU sampling of the app while scrolling.
	const sampler = Bun.spawn({
		cmd: [
			"sample",
			String(driver.pid),
			String(Math.ceil(SCROLL_SECONDS)),
			"-file",
			SAMPLE_OUT,
		],
		stdout: "pipe",
		stderr: "pipe",
	});

	if (SCROLL_HELPER) {
		const scroller = Bun.spawnSync({
			cmd: [SCROLL_HELPER, String(cx), String(cy), String(SCROLL_SECONDS)],
			stdout: "pipe",
			stderr: "pipe",
		});
		receipt.scroller = {
			exitCode: scroller.exitCode,
			stdout: scroller.stdout.toString().trim(),
			stderr: scroller.stderr.toString().trim(),
		};
	} else {
		// Fallback: drive protocol scroll commands rapidly.
		const positions = [0, 2, 4, 6, 7, 5, 3, 1];
		const deadline = Date.now() + SCROLL_SECONDS * 1000;
		let i = 0;
		while (Date.now() < deadline) {
			await driver.request(
				{
					type: "setAgentChatTranscriptScroll",
					itemIx: positions[i % positions.length],
					offsetPx: 0,
				},
				{ expect: "externalCommandResult", timeoutMs: 5_000 },
			);
			i += 1;
		}
		receipt.scroller = { protocolScrolls: i };
	}

	await sampler.exited;
	await Bun.sleep(400);

	receipt.renderTrace = parseRenderTrace(driver.logPath);
	receipt.samplePath = SAMPLE_OUT;
	receipt.pass = receipt.failures.length === 0;
} catch (error) {
	receipt.failures.push({
		name: "exception",
		message: error instanceof Error ? error.message : String(error),
	});
} finally {
	await driver.close();
	writeFileSync(RECEIPT, `${JSON.stringify(receipt, null, 2)}\n`);
	console.log(
		JSON.stringify(
			{
				pass: receipt.pass,
				renderTrace: receipt.renderTrace,
				scroller: receipt.scroller,
				failures: receipt.failures,
			},
			null,
			2,
		),
	);
}
process.exit(receipt.pass ? 0 : 1);

#!/usr/bin/env bun
import { chmod, mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { Driver, type Json } from "../devtools/driver.ts";

const binary =
	process.env.SCRIPT_KIT_GPUI_BINARY ??
	"target-agent/artifacts/agent-chat-recovery/script-kit-gpui";
const outputPath = resolve(
	process.env.PROBE_RECEIPT ?? ".test-output/agent-chat-retry-recovery.json",
);

function asArray(value: unknown): Json[] {
	return Array.isArray(value) ? (value as Json[]) : [];
}

function sleep(ms: number): Promise<void> {
	return new Promise((resolveSleep) => setTimeout(resolveSleep, ms));
}

function popupFromList(list: Json): Json | null {
	return (
		asArray(list.windows).find(
			(window) =>
				window.id === "confirm-popup" ||
				(window.kind === "promptPopup" &&
					window.semanticSurface === "confirmDialog"),
		) ?? null
	);
}

async function waitForPopup(
	driver: Driver,
	timeoutMs = 15_000,
	titleIncludes?: string,
): Promise<Json> {
	const deadline = Date.now() + timeoutMs;
	const poll = async (): Promise<Json> => {
		const last = await driver.listAutomationWindows({ timeoutMs: 5_000 });
		const popup = popupFromList(last);
		if (
			popup &&
			(!titleIncludes || String(popup.title ?? "").includes(titleIncludes))
		) {
			return popup;
		}
		if (Date.now() >= deadline) {
			throw new Error(`confirm-popup did not appear: ${JSON.stringify(last)}`);
		}
		await sleep(100);
		return poll();
	};
	return poll();
}

async function waitForNoPopup(
	driver: Driver,
	timeoutMs = 8_000,
): Promise<Json> {
	const deadline = Date.now() + timeoutMs;
	const poll = async (): Promise<Json> => {
		const last = await driver.listAutomationWindows({ timeoutMs: 5_000 });
		if (!popupFromList(last)) return last;
		if (Date.now() >= deadline) {
			throw new Error(`confirm-popup did not close: ${JSON.stringify(last)}`);
		}
		await sleep(100);
		return poll();
	};
	return poll();
}

async function popupDigest(driver: Driver): Promise<Json> {
	const elements = await driver.getElements(
		{ target: { type: "id", id: "confirm-popup" }, limit: 60 },
		{ timeoutMs: 8_000 },
	);
	return {
		focusedSemanticId: elements.focusedSemanticId ?? null,
		labels: asArray(elements.elements)
			.map((element) => element.label ?? element.text ?? element.value)
			.filter((value) => typeof value === "string"),
		elements: elements.elements ?? [],
	};
}

const dir = await mkdtemp(join(tmpdir(), "sk-agent-chat-recovery-"));
const readyMarker = join(dir, "ready");
const mockPi = join(dir, "pi");
await writeFile(
	mockPi,
	`#!/usr/bin/env bun
import { existsSync } from "node:fs";
const marker = ${JSON.stringify(readyMarker)};
let buffer = "";
for await (const chunk of Bun.stdin.stream()) {
  buffer += new TextDecoder().decode(chunk, { stream: true });
  let newline = buffer.indexOf("\\n");
  while (newline >= 0) {
    const line = buffer.slice(0, newline).trim();
    buffer = buffer.slice(newline + 1);
    newline = buffer.indexOf("\\n");
    if (!line) continue;
    const command = JSON.parse(line);
    if (!existsSync(marker)) continue;
    if (command.type === "get_available_models") {
      console.log(JSON.stringify({ type: "response", id: command.id, command: command.type, success: true, data: { models: [{ provider: "openai-codex", id: "gpt-5.4", name: "Mock GPT", contextWindow: 256000 }] } }));
    } else if (command.type === "set_model" || command.type === "abort") {
      console.log(JSON.stringify({ type: "response", id: command.id, command: command.type, success: true, data: {} }));
    } else if (command.type === "prompt") {
      console.log(JSON.stringify({ type: "message_update", assistantMessageEvent: { type: "text_delta", delta: "Recovery verified." } }));
      console.log(JSON.stringify({ type: "agent_end" }));
    }
  }
}
`,
);
await chmod(mockPi, 0o755);
await mkdir(resolve(".test-output"), { recursive: true });
await mkdir(resolve(".test-screenshots/agent-chat-retry-recovery"), {
	recursive: true,
});

const receipt: Json = {
	schemaVersion: 1,
	verifier: "agent-chat-retry-recovery-probe",
	binary,
	mockPi,
	status: "fail",
};

const driver = await Driver.launch({
	binary,
	sessionName: "agent-chat-retry-recovery",
	sandboxHome: true,
	readyTimeoutMs: 20_000,
	defaultTimeoutMs: 12_000,
	env: {
		SCRIPT_KIT_PI_BINARY: mockPi,
		SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
	},
});

try {
	await driver.request({ type: "show" });
	await driver.setFilterAndWait("recovery proof", { timeoutMs: 8_000 });
	await driver.waitForSettle();
	receipt.before = await driver.getState({ timeoutMs: 8_000 });

	// Startup prewarm must reach the terminal failed state before Cmd+Enter;
	// otherwise the ordinary low-latency path is allowed to join Preparing.
	await sleep(10_800);
	await driver.simulateGpuiEvent(
		{ type: "keyDown", key: "enter", modifiers: ["cmd"] },
		{ target: { type: "main" } },
	);

	receipt.failurePopup = await waitForPopup(driver);
	receipt.failureUi = await popupDigest(driver);
	receipt.failureScreenshot = await driver.captureScreenshot({
		target: { type: "id", id: "confirm-popup" },
		savePath: resolve(
			".test-screenshots/agent-chat-retry-recovery/failure-modal.png",
		),
		timeoutMs: 10_000,
	});
	const failureLabels = asArray(receipt.failureUi.labels).map(String).join(" ");
	if (
		!/Retry/i.test(failureLabels) ||
		!/Details/i.test(failureLabels) ||
		!/Back/i.test(failureLabels)
	) {
		throw new Error(`recovery actions missing: ${failureLabels}`);
	}

	await driver.simulateGpuiEvent(
		{ type: "keyDown", key: "i", modifiers: ["cmd"] },
		{ target: { type: "main" } },
	);
	receipt.detailsPopup = await waitForPopup(driver, 8_000, "details");
	receipt.detailsUi = await popupDigest(driver);
	receipt.detailsScreenshot = await driver.captureScreenshot({
		target: { type: "id", id: "confirm-popup" },
		savePath: resolve(
			".test-screenshots/agent-chat-retry-recovery/details-modal.png",
		),
		timeoutMs: 10_000,
	});
	const detailsLabels = asArray(receipt.detailsUi.labels).map(String).join(" ");
	if (!/Retry/i.test(detailsLabels) || !/Back/i.test(detailsLabels)) {
		throw new Error(`details actions missing: ${detailsLabels}`);
	}

	await driver.simulateGpuiEvent(
		{ type: "keyDown", key: "escape", modifiers: [] },
		{ target: { type: "main" } },
	);
	receipt.backToFailurePopup = await waitForPopup(
		driver,
		8_000,
		"couldn't start",
	);
	receipt.backToFailureUi = await popupDigest(driver);

	// Replace the sidecar behavior in place. Retry must spawn a fresh generation
	// and open Agent Chat only after that generation reports available models.
	await writeFile(readyMarker, "ready\n");
	await driver.simulateGpuiEvent(
		{ type: "keyDown", key: "enter", modifiers: [] },
		{ target: { type: "main" } },
	);
	await waitForNoPopup(driver, 12_000);

	const deadline = Date.now() + 12_000;
	const waitForAgentChat = async (): Promise<Json> => {
		const state = (await driver.getState({ timeoutMs: 8_000 })) as Json;
		const promptType = String(state.promptType ?? "").toLowerCase();
		const surface = String(
			(state.surfaceContract as Json | undefined)?.surface ?? "",
		).toLowerCase();
		if (promptType.includes("agent") || surface.includes("agent")) return state;
		if (Date.now() >= deadline) return state;
		await sleep(150);
		return waitForAgentChat();
	};
	const state = await waitForAgentChat();
	receipt.afterRetry = state;
	receipt.logs = await driver.getLogs({
		limit: 300,
		target: "script_kit::tab_ai",
	});
	const logText = JSON.stringify(receipt.logs);
	const opened =
		String(state.promptType ?? "")
			.toLowerCase()
			.includes("agent") ||
		String((state.surfaceContract as Json | undefined)?.surface ?? "")
			.toLowerCase()
			.includes("agent");
	if (!opened)
		throw new Error(
			`Agent Chat did not open after healthy retry: ${JSON.stringify(state)}`,
		);
	if (
		!logText.includes("agent_chat_recovery_start") ||
		!logText.includes("agent_chat_recovery_success")
	) {
		throw new Error("missing structured recovery start/success logs");
	}

	receipt.status = "pass";
} catch (error) {
	receipt.error = error instanceof Error ? error.message : String(error);
} finally {
	await driver.close();
	await writeFile(outputPath, `${JSON.stringify(receipt, null, 2)}\n`);
}

console.log(JSON.stringify(receipt, null, 2));
process.exit(receipt.status === "pass" ? 0 : 1);

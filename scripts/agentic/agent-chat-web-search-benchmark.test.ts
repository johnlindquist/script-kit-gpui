/// <reference types="bun-types" />
import { describe, expect, test } from "bun:test";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
	assessAnswer,
	codexExecCommand,
	loadQuickAiContract,
	parseCodexEvent,
	parsePiJsonEvent,
	parsePiRpcEvent,
	piExtensionCommand,
	piRpcCommand,
	type RunReceipt,
	summarizeRuns,
} from "./agent-chat-web-search-benchmark.ts";

describe("Quick AI web-search benchmark", () => {
	const contract = loadQuickAiContract();

	test("loads the live Quick AI model and web-only profile contract", () => {
		expect(contract.provider).toBe("openai-codex");
		expect(contract.model).toBe("gpt-5.3-codex-spark");
		expect(contract.tools).toEqual(["web_search"]);
		expect(contract.focusedSearchBudget).toBe(1);
		expect(contract.appendSystemPrompt).toContain("You are Quick AI");
		expect(contract.appendSystemPrompt).toContain(
			"exactly one web_search action containing one focused query",
		);
	});

	test("Pi RPC command matches the Quick AI launch flags", () => {
		const command = piRpcCommand(contract, "/tmp/pi");
		expect(command.executable).toBe("/tmp/pi");
		expect(command.args).toContain("rpc");
		expect(command.args).toContain("gpt-5.3-codex-spark");
		expect(command.args).toContain("web_search");
		expect(command.args).toContain("--no-context-files");
		expect(command.args).toContain("--no-session");
	});

	test("Pi extension command loads only web_search from one explicit extension", () => {
		const command = piExtensionCommand(
			contract,
			"pi",
			"/tmp/pi-web-access/index.ts",
			"query",
		);
		expect(command.args).toContain("gpt-5.3-codex-spark");
		expect(command.args).toContain("web_search");
		expect(command.args).toContain("--no-builtin-tools");
		expect(command.args).toContain("--no-extensions");
		expect(command.args).toContain("/tmp/pi-web-access/index.ts");
		expect(command.args).toContain("--no-context-files");
		expect(command.args).toContain("--no-session");
	});

	test("Codex command is native-search, ephemeral, read-only, and model-matched", () => {
		const command = codexExecCommand(contract, "codex", "/tmp/empty", "query");
		expect(command.args[0]).toBe("--search");
		expect(command.args).toContain("--disable");
		expect(command.args).toContain("plugins");
		expect(command.args).toContain("skills.bundled.enabled=false");
		expect(command.args).toContain('model_reasoning_effort="low"');
		expect(command.args).toContain('tools.web_search.context_size="low"');
		expect(command.args).toContain("gpt-5.3-codex-spark");
		expect(command.args).toContain("read-only");
		expect(command.args).toContain("--ephemeral");
		expect(command.args).toContain("--ignore-user-config");
		expect(command.args).toContain("--output-schema");
		const schemaPath =
			command.args[command.args.indexOf("--output-schema") + 1];
		const schema = JSON.parse(readFileSync(schemaPath, "utf8"));
		expect(JSON.stringify(schema)).not.toContain("uniqueItems");
		expect(JSON.stringify(schema)).not.toContain("maxItems");
		expect(JSON.stringify(schema)).not.toContain("maxLength");
		expect(
			command.args.find((arg) => arg.startsWith("developer_instructions=")),
		).toContain("You are Quick AI");
	});

	test("parses Pi and Codex native search and answer events", () => {
		expect(
			parsePiRpcEvent({ type: "tool_execution_start", toolName: "web_search" })
				.webSearch,
		).toBe(true);
		expect(
			parsePiRpcEvent({
				type: "message_update",
				assistantMessageEvent: { type: "text_delta", delta: "hello" },
			}).answerDelta,
		).toBe("hello");
		expect(
			parsePiJsonEvent({
				type: "message_end",
				message: {
					role: "assistant",
					content: [
						{ type: "toolCall", name: "web_search" },
						{ type: "text", text: "extension answer" },
					],
				},
			}).webSearch,
		).toBe(true);
		expect(
			parsePiJsonEvent({
				type: "message_end",
				message: {
					role: "assistant",
					content: [{ type: "text", text: "extension answer" }],
				},
			}).answerDelta,
		).toBe("extension answer");
		expect(
			parseCodexEvent({
				type: "item.started",
				item: { type: "web_search", query: "Rust" },
			}).webSearch,
		).toBe(true);
		expect(
			parseCodexEvent({
				type: "item.completed",
				item: { type: "agent_message", text: "answer" },
			}).answerDelta,
		).toBe("answer");
	});

	test("Codex parser recognizes only exact web-search items and structured actions", () => {
		expect(
			parseCodexEvent({
				type: "item.completed",
				item: { type: "agent_message", text: "I used web_search" },
			}).webSearch,
		).toBe(false);
		expect(
			parseCodexEvent({
				type: "item.completed",
				item: { type: "command_execution", output: "web_search" },
			}).webSearch,
		).toBe(false);
		const source = parseCodexEvent({
			type: "item.completed",
			item: {
				type: "web_search",
				action: {
					type: "open_page",
					url: "https://blog.rust-lang.org/source",
				},
			},
		});
		expect(source.webSearch).toBe(true);
		expect(source.structuredSourceUrls).toEqual([
			"https://blog.rust-lang.org/source",
		]);
		expect(
			parseCodexEvent({
				type: "item.completed",
				item: {
					type: "web_search",
					query: "https://blog.rust-lang.org/releases/latest/",
					action: { type: "other" },
				},
			}).structuredSourceUrls,
		).toEqual(["https://blog.rust-lang.org/releases/latest/"]);
		expect(
			parseCodexEvent({
				type: "item.completed",
				item: {
					id: "warning",
					type: "error",
					message: "Skill descriptions were shortened",
				},
			}).nonSearchTool,
		).toBeNull();
		expect(
			parseCodexEvent({
				type: "item.completed",
				item: {
					type: "agent_message",
					text: "https://blog.rust-lang.org/not-structured",
				},
			}).structuredSourceUrls,
		).toEqual([]);
		const structuredAnswer = parseCodexEvent({
			type: "item.completed",
			item: {
				type: "agent_message",
				text: JSON.stringify({
					answer: "Rust 1.97.0",
					sources: ["https://blog.rust-lang.org/releases/latest/"],
				}),
			},
		});
		expect(structuredAnswer.answerDelta).toContain("Rust 1.97.0");
		expect(structuredAnswer.structuredSourceUrls).toEqual([
			"https://blog.rust-lang.org/releases/latest/",
		]);
		expect(
			parseCodexEvent({
				type: "item.started",
				item: { type: "future_tool" },
			}).nonSearchTool,
		).toBe("future_tool");
	});

	test("requires a sourced, non-failure answer for ranking", () => {
		expect(
			assessAnswer("Rust 1.97 https://blog.rust-lang.org/release", [
				"Rust 1.97",
			]),
		).toEqual({
			useful: true,
			sourceUrlCount: 1,
			expectedPatternsMatched: true,
		});
		expect(
			assessAnswer("Rust unknown https://blog.rust-lang.org/release", [
				"Rust 1.97",
			]).useful,
		).toBe(false);
		expect(
			assessAnswer("No usable results; check https://blog.rust-lang.org/")
				.useful,
		).toBe(false);
		expect(
			assessAnswer(
				"The search did not return a usable official result; check https://blog.rust-lang.org/",
			).useful,
		).toBe(false);
		expect(
			assessAnswer(
				"I can't reliably provide the date; see https://blog.rust-lang.org/",
			).useful,
		).toBe(false);

		const run = (
			path: RunReceipt["path"],
			total: number,
			search = true,
			useful = true,
		): RunReceipt => ({
			type: "agent-chat.web-search-benchmark.run.v1",
			path,
			trial: 1,
			query: "q",
			status: "ok",
			startedAt: "2026-07-15T00:00:00Z",
			command: { executable: path, args: [] },
			timingsMs: {
				processSpawn: 1,
				firstEvent: 2,
				firstWebSearch: search ? 3 : null,
				firstAnswer: 4,
				total,
			},
			webSearchObserved: search,
			usefulAnswerObserved: useful,
			sourceUrlCount: useful ? 1 : 0,
			expectedPatternsMatched: useful,
			answer: useful ? "answer https://example.com" : "No usable results",
			answerChars: useful ? 26 : 17,
			exitCode: 0,
			error: null,
		});
		const summary = summarizeRuns([
			run("pi-rpc-cold", 50),
			run("pi-rpc-cold", 70),
			run("codex-exec", 40),
			run("pi-rpc-warm", 1, true, false),
		]);
		expect(summary.winner).toBe("codex-exec");
		expect(
			summary.summaries.find((item) => item.path === "pi-rpc-cold")
				?.medianTotalMs,
		).toBe(60);
		expect(
			summary.summaries.find((item) => item.path === "pi-rpc-warm")?.valid,
		).toBe(0);
	});

	test("standalone CLI records cold, warm, and Codex receipts with fake native processes", () => {
		const temp = mkdtempSync(
			join(tmpdir(), "agent-chat-web-search-benchmark-"),
		);
		const fakePi = join(temp, "fake-pi.ts");
		const fakeCodex = join(temp, "fake-codex.ts");
		const output = join(temp, "receipt.json");
		writeFileSync(
			fakePi,
			`#!/usr/bin/env bun
const reader = Bun.stdin.stream().getReader();
const decoder = new TextDecoder();
if (Bun.argv.includes("json")) {
  console.log(JSON.stringify({ type: "message_end", message: { role: "assistant", model: "gpt-5.3-codex-spark", content: [{ type: "toolCall", name: "web_search" }, { type: "text", text: "extension answer" }] } }));
  console.log(JSON.stringify({ type: "agent_end" }));
  process.exit(0);
}
let buffered = "";
while (true) {
  const { done, value } = await reader.read();
  if (done) break;
  buffered += decoder.decode(value, { stream: true });
  let newline;
  while ((newline = buffered.indexOf("\\n")) >= 0) {
    const line = buffered.slice(0, newline); buffered = buffered.slice(newline + 1);
    if (!line.trim()) continue;
    const command = JSON.parse(line);
    if (command.type === "get_available_models") {
      console.log(JSON.stringify({ type: "response", id: command.id, command: command.type, success: true, data: { models: [] } }));
    } else if (command.type === "prompt") {
      console.log(JSON.stringify({ type: "tool_execution_start", toolName: "web_search" }));
      console.log(JSON.stringify({ type: "message_update", assistantMessageEvent: { type: "text_delta", delta: "pi answer" } }));
      console.log(JSON.stringify({ type: "agent_end" }));
    }
  }
}
`,
		);
		writeFileSync(
			fakeCodex,
			`#!/usr/bin/env bun
console.log(JSON.stringify({ type: "item.started", item: { type: "web_search", query: "fixture" } }));
console.log(JSON.stringify({ type: "item.completed", item: { type: "agent_message", text: "codex answer" } }));
console.log(JSON.stringify({ type: "turn.completed" }));
`,
		);
		chmodSync(fakePi, 0o755);
		chmodSync(fakeCodex, 0o755);

		const result = Bun.spawnSync({
			cmd: [
				"bun",
				join(import.meta.dir, "agent-chat-web-search-benchmark.ts"),
				"--trials",
				"2",
				"--timeout-ms",
				"5000",
				"--pi-binary",
				fakePi,
				"--pi-js-binary",
				fakePi,
				"--codex-binary",
				fakeCodex,
				"--output",
				output,
			],
			stdout: "pipe",
			stderr: "pipe",
		});
		if (result.exitCode !== 0) {
			throw new Error(result.stderr.toString() || result.stdout.toString());
		}
		let receipt: { runs: RunReceipt[] };
		try {
			receipt = JSON.parse(readFileSync(output, "utf8"));
		} catch (error) {
			throw new Error(`Failed to parse benchmark receipt at ${output}`, {
				cause: error,
			});
		}
		expect(receipt.runs).toHaveLength(8);
		expect(receipt.runs.every((run: RunReceipt) => run.status === "ok")).toBe(
			true,
		);
		expect(
			receipt.runs.filter((run: RunReceipt) => run.path === "pi-rpc-warm"),
		).toHaveLength(2);
		expect(
			receipt.runs.filter(
				(run: RunReceipt) => run.path === "pi-extension-cold",
			),
		).toHaveLength(2);
		expect(receipt.runs.every((run: RunReceipt) => run.webSearchObserved)).toBe(
			true,
		);
		expect(
			receipt.runs.every((run: RunReceipt) => !run.usefulAnswerObserved),
		).toBe(true);
	});
});

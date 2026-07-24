#!/usr/bin/env bun
/// <reference types="bun-types" />

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

export const REPO_ROOT = resolve(import.meta.dir, "../..");
export const PROFILE_SOURCE = join(REPO_ROOT, "src/ai/agent_chat/profiles.rs");
const QUICK_AI_OUTPUT_SCHEMA = {
	type: "object",
	additionalProperties: false,
	properties: {
		answer: { type: "string" },
		sources: { type: "array", items: { type: "string" } },
	},
	required: ["answer", "sources"],
};

export type BenchmarkPath =
	| "pi-rpc-cold"
	| "pi-rpc-warm"
	| "pi-extension-cold"
	| "codex-exec";

export type QuickAiContract = {
	provider: string;
	model: string;
	appendSystemPrompt: string;
	tools: string[];
	focusedSearchBudget: number;
	profileSource: string;
};

export type TimingReceipt = {
	processSpawn: number;
	firstEvent: number | null;
	firstWebSearch: number | null;
	firstAnswer: number | null;
	total: number;
};

export type RunReceipt = {
	type: "agent-chat.web-search-benchmark.run.v1";
	path: BenchmarkPath;
	trial: number;
	query: string;
	status: "ok" | "error" | "dry-run";
	startedAt: string;
	command: { executable: string; args: string[] };
	timingsMs: TimingReceipt;
	webSearchObserved: boolean;
	usefulAnswerObserved: boolean;
	sourceUrlCount: number;
	expectedPatternsMatched: boolean;
	answer: string;
	answerChars: number;
	exitCode: number | null;
	error: string | null;
};

type ParsedEvent = {
	webSearch: boolean;
	answerDelta: string;
	terminal: boolean;
	error: string | null;
	structuredSourceUrls?: string[];
	nonSearchTool?: string | null;
};

type RunnerOptions = {
	paths: BenchmarkPath[];
	queries: string[];
	trials: number;
	timeoutMs: number;
	output: string | null;
	dryRun: boolean;
	piBinary: string;
	piJsBinary: string;
	piExtension: string;
	codexBinary: string;
	expectedPatterns: string[];
};

const DEFAULT_QUERY =
	"Search the web for the latest stable Rust release. Answer with the version, release date, and one official source URL.";

function rustStringConstant(source: string, name: string): string {
	const declaration = `pub const ${name}: &str = `;
	const declarationStart = source.indexOf(declaration);
	if (declarationStart < 0) {
		throw new Error(
			`Missing Rust string constant ${name} in ${PROFILE_SOURCE}`,
		);
	}
	const valueStart = declarationStart + declaration.length;
	if (source[valueStart] !== '"') {
		throw new Error(
			`Rust constant ${name} is not a quoted string in ${PROFILE_SOURCE}`,
		);
	}
	let escaped = false;
	let valueEnd = -1;
	for (let index = valueStart + 1; index < source.length; index++) {
		const character = source[index];
		if (escaped) {
			escaped = false;
			continue;
		}
		if (character === "\\") {
			escaped = true;
			continue;
		}
		if (character === '"') {
			valueEnd = index + 1;
			break;
		}
	}
	if (valueEnd < 0) {
		throw new Error(
			`Unterminated Rust string constant ${name} in ${PROFILE_SOURCE}`,
		);
	}
	try {
		return JSON.parse(source.slice(valueStart, valueEnd));
	} catch (error) {
		throw new Error(
			`Invalid Rust string constant ${name} in ${PROFILE_SOURCE}`,
			{ cause: error },
		);
	}
}

export function loadQuickAiContract(
	sourcePath = PROFILE_SOURCE,
): QuickAiContract {
	const source = readFileSync(sourcePath, "utf8");
	const profileBody = source.match(
		/pub fn built_in_quick_ai_profile[\s\S]*?\n}\n\npub fn built_in_profiles/,
	)?.[0];
	if (!profileBody)
		throw new Error(`Missing built_in_quick_ai_profile in ${sourcePath}`);

	const requiredProfileFragments = [
		"provider: Some(DEFAULT_PI_PROVIDER.to_string())",
		"model: Some(QUICK_AI_PI_MODEL.to_string())",
		"tools: Some(QUICK_AI_PI_TOOLS.iter()",
		"disable_extensions: Some(true)",
		"disable_skills: Some(true)",
		"disable_prompt_templates: Some(true)",
		"disable_context_files: Some(true)",
		"hide_cwd_in_prompt: Some(true)",
		"no_session: Some(true)",
	];
	for (const fragment of requiredProfileFragments) {
		if (!profileBody.includes(fragment)) {
			throw new Error(`Quick AI profile contract changed; missing ${fragment}`);
		}
	}

	const toolsBody = source.match(
		/pub const QUICK_AI_PI_TOOLS: \[&str; \d+] = \[([^\]]*)\];/,
	)?.[1];
	if (!toolsBody) throw new Error(`Missing QUICK_AI_PI_TOOLS in ${sourcePath}`);
	const tools = [...toolsBody.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
	const focusedSearchBudget = Number(
		source.match(/pub const QUICK_AI_FOCUSED_SEARCH_BUDGET: u8 = (\d+);/)?.[1],
	);
	if (!Number.isInteger(focusedSearchBudget)) {
		throw new Error(`Missing QUICK_AI_FOCUSED_SEARCH_BUDGET in ${sourcePath}`);
	}

	return {
		provider: rustStringConstant(source, "DEFAULT_PI_PROVIDER"),
		model: rustStringConstant(source, "QUICK_AI_PI_MODEL"),
		appendSystemPrompt: rustStringConstant(
			source,
			"QUICK_AI_APPEND_SYSTEM_PROMPT",
		),
		tools,
		focusedSearchBudget,
		profileSource: sourcePath,
	};
}

export function piRpcCommand(
	contract: QuickAiContract,
	piBinary: string,
): { executable: string; args: string[] } {
	return {
		executable: piBinary,
		args: [
			"--mode",
			"rpc",
			"--provider",
			contract.provider,
			"--model",
			contract.model,
			"--append-system-prompt",
			contract.appendSystemPrompt,
			"--tools",
			contract.tools.join(","),
			"--no-extensions",
			"--no-skills",
			"--no-prompt-templates",
			"--no-context-files",
			"--no-session",
		],
	};
}

export function piExtensionCommand(
	contract: QuickAiContract,
	piBinary: string,
	extension: string,
	query: string,
): { executable: string; args: string[] } {
	return {
		executable: piBinary,
		args: [
			"--mode",
			"json",
			"--provider",
			contract.provider,
			"--model",
			contract.model,
			"--append-system-prompt",
			contract.appendSystemPrompt,
			"--tools",
			contract.tools.join(","),
			"--no-builtin-tools",
			"--no-extensions",
			"--extension",
			extension,
			"--no-skills",
			"--no-prompt-templates",
			"--no-context-files",
			"--no-themes",
			"--no-session",
			"--no-approve",
			query,
		],
	};
}

function tomlString(value: string): string {
	return JSON.stringify(value);
}

export function codexExecCommand(
	contract: QuickAiContract,
	codexBinary: string,
	cwd: string,
	query: string,
): { executable: string; args: string[] } {
	mkdirSync(cwd, { recursive: true });
	const outputSchemaPath = join(cwd, "quick-ai-output-schema.json");
	writeFileSync(outputSchemaPath, JSON.stringify(QUICK_AI_OUTPUT_SCHEMA));
	return {
		executable: codexBinary,
		args: [
			"--search",
			"--disable",
			"plugins",
			"--config",
			"skills.bundled.enabled=false",
			"--config",
			'model_reasoning_effort="low"',
			"--config",
			'tools.web_search.context_size="low"',
			"--model",
			contract.model,
			"--sandbox",
			"read-only",
			"--cd",
			cwd,
			"--config",
			`developer_instructions=${tomlString(contract.appendSystemPrompt)}`,
			"exec",
			"--ephemeral",
			"--ignore-user-config",
			"--ignore-rules",
			"--skip-git-repo-check",
			"--output-schema",
			outputSchemaPath,
			"--json",
			query,
		],
	};
}

export function assessAnswer(
	answer: string,
	expectedPatterns: string[] = [],
): {
	useful: boolean;
	sourceUrlCount: number;
	expectedPatternsMatched: boolean;
} {
	const sourceUrlCount = answer.match(/https?:\/\/[^\s)\]}]+/g)?.length ?? 0;
	const normalized = answer.toLowerCase().replaceAll("’", "'");
	const failureMarkers = [
		"no usable results",
		"no reliable official source",
		"couldn't find",
		"could not find",
		"can't reliably provide",
		"cannot reliably provide",
		"incomplete-handoff",
	];
	const failurePattern =
		/\b(?:no|not|did not|didn't|can't|cannot|could not|couldn't)\b.{0,80}\b(?:usable|useful|reliable|verify|provide)\b/is;
	const expectedPatternsMatched = expectedPatterns.every((expectation) =>
		expectation
			.split("|")
			.map((alternative) => alternative.trim().toLowerCase())
			.filter(Boolean)
			.some((alternative) => normalized.includes(alternative)),
	);
	return {
		useful:
			sourceUrlCount > 0 &&
			expectedPatternsMatched &&
			!failureMarkers.some((marker) => normalized.includes(marker)) &&
			!failurePattern.test(normalized),
		sourceUrlCount,
		expectedPatternsMatched,
	};
}

function recursivelyContains(value: unknown, needle: string): boolean {
	if (typeof value === "string") return value.toLowerCase().includes(needle);
	if (Array.isArray(value))
		return value.some((item) => recursivelyContains(item, needle));
	if (value && typeof value === "object") {
		return Object.entries(value).some(
			([key, item]) =>
				key.toLowerCase().includes(needle) || recursivelyContains(item, needle),
		);
	}
	return false;
}

export function parsePiRpcEvent(value: unknown): ParsedEvent {
	if (!value || typeof value !== "object") {
		return { webSearch: false, answerDelta: "", terminal: false, error: null };
	}
	const event = value as Record<string, any>;
	const update = event.assistantMessageEvent ?? event.messageEvent ?? event;
	const answerDelta =
		event.type === "message_update" && update?.type === "text_delta"
			? String(update.delta ?? update.text ?? "")
			: "";
	const error =
		event.type === "agent_end" &&
		typeof event.error === "string" &&
		event.error.trim()
			? event.error
			: null;
	return {
		webSearch:
			[
				"tool_call_end",
				"tool_execution_start",
				"tool_execution_update",
				"tool_execution_end",
			].includes(event.type) && recursivelyContains(event, "web_search"),
		answerDelta,
		terminal: event.type === "agent_end",
		error,
	};
}

export function parseCodexEvent(value: unknown): ParsedEvent {
	if (!value || typeof value !== "object") {
		return { webSearch: false, answerDelta: "", terminal: false, error: null };
	}
	const event = value as Record<string, any>;
	const item =
		event.item && typeof event.item === "object"
			? (event.item as Record<string, any>)
			: {};
	const isItemEvent = [
		"item.started",
		"item.updated",
		"item.completed",
	].includes(String(event.type ?? ""));
	const exactWebSearch = isItemEvent && item.type === "web_search";
	const action =
		exactWebSearch && item.action && typeof item.action === "object"
			? (item.action as Record<string, any>)
			: {};
	const actionUrl =
		exactWebSearch &&
		["open_page", "find_in_page"].includes(String(action.type ?? "")) &&
		typeof action.url === "string"
			? action.url
			: null;
	const queryUrl =
		exactWebSearch && typeof item.query === "string" ? item.query : null;
	const structuredUrl = [actionUrl, queryUrl].find(
		(url): url is string =>
			typeof url === "string" && /^https?:\/\/[^\s/]+/i.test(url),
	);
	const structuredSourceUrls = structuredUrl ? [structuredUrl] : [];
	let answerDelta =
		event.type === "item.completed" && item.type === "agent_message"
			? String(item.text ?? "")
			: "";
	if (answerDelta.startsWith("{")) {
		try {
			const output = JSON.parse(answerDelta);
			const sources = Array.isArray(output?.sources)
				? output.sources.filter(
						(source: unknown): source is string =>
							typeof source === "string" && /^https?:\/\//i.test(source),
					)
				: [];
			if (typeof output?.answer === "string" && sources.length > 0) {
				answerDelta = `${output.answer.trim()}\n\n${sources
					.map((source: string) => `Source: ${source}`)
					.join("\n")}`;
				structuredSourceUrls.push(...sources);
			}
		} catch {
			// A plain-text answer beginning with `{` remains plain text.
		}
	}
	const error =
		event.type === "turn.failed" || event.type === "error"
			? String(event.error?.message ?? event.message ?? "Codex turn failed")
			: null;
	return {
		webSearch: exactWebSearch,
		answerDelta,
		terminal: event.type === "turn.completed" || event.type === "turn.failed",
		error,
		structuredSourceUrls,
		nonSearchTool:
			isItemEvent &&
			typeof item.type === "string" &&
			![
				"agent_message",
				"reasoning",
				"todo_list",
				"web_search",
				"error",
			].includes(item.type)
				? item.type
				: null,
	};
}

export function parsePiJsonEvent(value: unknown): ParsedEvent {
	if (!value || typeof value !== "object") {
		return { webSearch: false, answerDelta: "", terminal: false, error: null };
	}
	const event = value as Record<string, any>;
	const message = event.message ?? {};
	const answerDelta =
		event.type === "message_end" && message.role === "assistant"
			? (message.content ?? [])
					.filter((part: Record<string, unknown>) => part.type === "text")
					.map((part: Record<string, unknown>) => String(part.text ?? ""))
					.join("")
			: "";
	const error =
		event.type === "message_end" &&
		message.role === "assistant" &&
		message.stopReason === "error"
			? String(message.errorMessage ?? "Pi extension turn failed")
			: null;
	return {
		webSearch:
			(event.type === "tool_execution_start" ||
				event.type === "tool_execution_end" ||
				event.type === "message_end") &&
			recursivelyContains(event, "web_search"),
		answerDelta,
		terminal: event.type === "agent_end",
		error,
	};
}

function redactCommand(command: { executable: string; args: string[] }) {
	return {
		executable: command.executable,
		args: command.args.map((arg, index) => {
			if (arg.startsWith("developer_instructions=")) {
				return "developer_instructions=<quick-ai-source>";
			}
			if (command.args[index - 1] === "--append-system-prompt")
				return "<quick-ai-source>";
			return arg;
		}),
	};
}

function nowMs(start: number): number {
	return Math.round((performance.now() - start) * 100) / 100;
}

async function readJsonLines(
	stream: ReadableStream<Uint8Array>,
	onValue: (value: unknown) => void,
): Promise<string> {
	const reader = stream.getReader();
	const decoder = new TextDecoder();
	let buffered = "";
	let raw = "";
	while (true) {
		const { done, value } = await reader.read();
		if (done) break;
		const text = decoder.decode(value, { stream: true });
		raw += text;
		buffered += text;
		let newline = buffered.indexOf("\n");
		while (newline >= 0) {
			const line = buffered.slice(0, newline).trim();
			buffered = buffered.slice(newline + 1);
			if (line) {
				try {
					onValue(JSON.parse(line));
				} catch {
					// Preserve non-JSON diagnostics in `raw`; they are reported on failure.
				}
			}
			newline = buffered.indexOf("\n");
		}
	}
	const tail = buffered.trim();
	if (tail) {
		try {
			onValue(JSON.parse(tail));
		} catch {
			// Preserve it in `raw`.
		}
	}
	return raw;
}

async function withTimeout<T>(
	promise: Promise<T>,
	timeoutMs: number,
	label: string,
): Promise<T> {
	let timer: ReturnType<typeof setTimeout> | undefined;
	try {
		return await Promise.race([
			promise,
			new Promise<T>((_, reject) => {
				timer = setTimeout(
					() => reject(new Error(`${label} timed out after ${timeoutMs}ms`)),
					timeoutMs,
				);
			}),
		]);
	} finally {
		if (timer) clearTimeout(timer);
	}
}

async function writeChildStdin(stdin: any, text: string): Promise<void> {
	if (typeof stdin?.write === "function") {
		stdin.write(text);
		await stdin.flush?.();
		return;
	}
	if (typeof stdin?.getWriter === "function") {
		const writer = stdin.getWriter();
		try {
			await writer.write(new TextEncoder().encode(text));
		} finally {
			writer.releaseLock();
		}
		return;
	}
	throw new Error("Child stdin is not writable");
}

type LiveRun = {
	receipt: RunReceipt;
};

type PersistentPi = {
	process: ReturnType<typeof Bun.spawn>;
	stdin: any;
	setConsumer: (consumer: (value: unknown) => void) => void;
	stdout: Promise<string>;
	stderr: Promise<string>;
};

async function startPersistentPi(
	command: { executable: string; args: string[] },
	cwd: string,
	timeoutMs: number,
): Promise<PersistentPi> {
	const child = Bun.spawn({
		cmd: [command.executable, ...command.args],
		cwd,
		stdin: "pipe",
		stdout: "pipe",
		stderr: "pipe",
		env: process.env,
	});
	let consumer: (value: unknown) => void = () => {};
	const persistent: PersistentPi = {
		process: child,
		stdin: child.stdin,
		setConsumer: (next) => (consumer = next),
		stdout: readJsonLines(child.stdout, (value) => consumer(value)),
		stderr: new Response(child.stderr).text(),
	};

	let readyResolve: (() => void) | null = null;
	const ready = new Promise<void>(
		(resolveReady) => (readyResolve = resolveReady),
	);
	persistent.setConsumer((value) => {
		const event = value as Record<string, any>;
		if (
			event?.type === "response" &&
			event?.id === "benchmark-warmup" &&
			event?.success === true
		) {
			readyResolve?.();
		}
	});
	await writeChildStdin(
		child.stdin,
		`${JSON.stringify({ id: "benchmark-warmup", type: "get_available_models" })}\n`,
	);
	await withTimeout(ready, timeoutMs, "pi-rpc-warm startup");
	persistent.setConsumer(() => {});
	return persistent;
}

async function runProcess(
	path: BenchmarkPath,
	trial: number,
	query: string,
	command: { executable: string; args: string[] },
	cwd: string,
	timeoutMs: number,
	parser: (value: unknown) => ParsedEvent,
	rpc: boolean,
	expectedPatterns: string[],
	existing?: PersistentPi,
): Promise<LiveRun> {
	const startedAt = new Date().toISOString();
	const start = performance.now();
	let firstEvent: number | null = null;
	let firstWebSearch: number | null = null;
	let firstAnswer: number | null = null;
	let webSearchObserved = false;
	let answer = "";
	let eventError: string | null = null;
	let terminalResolve: (() => void) | null = null;
	const terminal = new Promise<void>(
		(resolveTerminal) => (terminalResolve = resolveTerminal),
	);

	const spawned = existing
		? null
		: Bun.spawn({
				cmd: [command.executable, ...command.args],
				cwd,
				stdin: rpc ? "pipe" : "ignore",
				stdout: "pipe",
				stderr: "pipe",
				env: process.env,
			});
	const childProcess = existing?.process ?? spawned;
	if (!childProcess) throw new Error(`Failed to create process for ${path}`);
	const processSpawn = existing ? 0 : nowMs(start);

	const consume = (value: unknown) => {
		const elapsed = nowMs(start);
		if (firstEvent === null) firstEvent = elapsed;
		const event = parser(value);
		if (event.webSearch) {
			webSearchObserved = true;
			if (firstWebSearch === null) firstWebSearch = elapsed;
		}
		if (event.answerDelta) {
			answer += event.answerDelta;
			if (firstAnswer === null) firstAnswer = elapsed;
		}
		if (event.error) eventError = event.error;
		if (event.terminal) terminalResolve?.();
	};

	if (existing) existing.setConsumer(consume);
	const stdoutPromise = spawned
		? readJsonLines(spawned.stdout, consume)
		: Promise.resolve("");
	const stderrPromise = spawned
		? new Response(spawned.stderr).text()
		: Promise.resolve("");

	if (rpc) {
		const stdin = existing?.stdin ?? childProcess.stdin;
		await writeChildStdin(
			stdin,
			`${JSON.stringify({ id: `benchmark-${trial}`, type: "prompt", message: query })}\n`,
		);
	}

	let exitCode: number | null = null;
	let error: string | null = null;
	try {
		if (rpc) {
			await withTimeout(terminal, timeoutMs, path);
			if (!existing) {
				childProcess.kill();
				exitCode = await childProcess.exited;
			}
		} else {
			exitCode = await withTimeout(childProcess.exited, timeoutMs, path);
		}
	} catch (caught) {
		error = caught instanceof Error ? caught.message : String(caught);
		childProcess.kill();
		exitCode = await childProcess.exited;
	}

	const [stdout, stderr] = await Promise.all([stdoutPromise, stderrPromise]);
	if (existing) existing.setConsumer(() => {});
	error ??= eventError;
	if (!error && !rpc && exitCode !== 0) {
		error = `exit ${exitCode}: ${stderr.trim() || stdout.trim() || "no diagnostics"}`;
	}
	if (!error && !webSearchObserved)
		error = "No native web_search event was observed";
	if (!error && !answer.trim()) error = "No assistant answer was observed";
	const answerAssessment = assessAnswer(answer, expectedPatterns);

	return {
		receipt: {
			type: "agent-chat.web-search-benchmark.run.v1",
			path,
			trial,
			query,
			status: error ? "error" : "ok",
			startedAt,
			command: redactCommand(command),
			timingsMs: {
				processSpawn,
				firstEvent,
				firstWebSearch,
				firstAnswer,
				total: nowMs(start),
			},
			webSearchObserved,
			usefulAnswerObserved: answerAssessment.useful,
			sourceUrlCount: answerAssessment.sourceUrlCount,
			expectedPatternsMatched: answerAssessment.expectedPatternsMatched,
			answer: answer.trim(),
			answerChars: answer.trim().length,
			exitCode,
			error,
		},
	};
}

function median(values: number[]): number | null {
	if (!values.length) return null;
	const sorted = [...values].sort((a, b) => a - b);
	const middle = Math.floor(sorted.length / 2);
	return sorted.length % 2
		? sorted[middle]
		: (sorted[middle - 1] + sorted[middle]) / 2;
}

export function summarizeRuns(runs: RunReceipt[]) {
	const paths = [...new Set(runs.map((run) => run.path))];
	const summaries = paths.map((path) => {
		const pathRuns = runs.filter((run) => run.path === path);
		const valid = pathRuns.filter(
			(run) =>
				run.status === "ok" &&
				run.webSearchObserved &&
				run.usefulAnswerObserved &&
				run.timingsMs.firstAnswer !== null,
		);
		return {
			path,
			attempted: pathRuns.length,
			valid: valid.length,
			medianTotalMs: median(valid.map((run) => run.timingsMs.total)),
			medianFirstAnswerMs: median(
				valid.flatMap((run) =>
					run.timingsMs.firstAnswer === null ? [] : [run.timingsMs.firstAnswer],
				),
			),
		};
	});
	const ranked = summaries
		.filter((summary) => summary.medianTotalMs !== null)
		.sort(
			(a, b) =>
				(a.medianTotalMs ?? Number.POSITIVE_INFINITY) -
				(b.medianTotalMs ?? Number.POSITIVE_INFINITY),
		);
	return { summaries, winner: ranked[0]?.path ?? null };
}

function argValues(name: string): string[] {
	const values: string[] = [];
	for (let index = 0; index < process.argv.length; index++) {
		if (process.argv[index] === name && process.argv[index + 1])
			values.push(process.argv[index + 1]);
	}
	return values;
}

function argValue(name: string, fallback: string): string {
	return argValues(name).at(-1) ?? fallback;
}

function parseOptions(): RunnerOptions {
	const paths = argValue(
		"--paths",
		"pi-rpc-cold,pi-rpc-warm,pi-extension-cold,codex-exec",
	)
		.split(",")
		.filter(Boolean) as BenchmarkPath[];
	const validPaths: BenchmarkPath[] = [
		"pi-rpc-cold",
		"pi-rpc-warm",
		"pi-extension-cold",
		"codex-exec",
	];
	if (paths.some((path) => !validPaths.includes(path))) {
		throw new Error(`--paths must contain only ${validPaths.join(",")}`);
	}
	const trials = Number(argValue("--trials", "3"));
	const timeoutMs = Number(argValue("--timeout-ms", "120000"));
	if (!Number.isInteger(trials) || trials < 1)
		throw new Error("--trials must be a positive integer");
	if (!Number.isFinite(timeoutMs) || timeoutMs < 1)
		throw new Error("--timeout-ms must be positive");
	const expectedPatterns = argValues("--expect");
	if (
		expectedPatterns.some(
			(expectation) => !expectation.split("|").some((part) => part.trim()),
		)
	) {
		throw new Error(
			"--expect must contain at least one non-empty literal alternative",
		);
	}
	const npmRootResult = Bun.spawnSync({
		cmd: ["npm", "root", "-g"],
		stdout: "pipe",
		stderr: "ignore",
	});
	const npmRoot = npmRootResult.success
		? npmRootResult.stdout.toString().trim()
		: "";
	const defaultExtension = npmRoot
		? join(npmRoot, "pi-web-access", "index.ts")
		: "pi-web-access/index.ts";
	const piExtension = argValue("--pi-extension", defaultExtension);
	if (paths.includes("pi-extension-cold") && !existsSync(piExtension)) {
		throw new Error(
			`Pi extension entrypoint not found at ${piExtension}; pass --pi-extension`,
		);
	}
	return {
		paths,
		queries: argValues("--query").length
			? argValues("--query")
			: [DEFAULT_QUERY],
		trials,
		timeoutMs,
		output: argValues("--output").at(-1) ?? null,
		dryRun: process.argv.includes("--dry-run"),
		piBinary: argValue("--pi-binary", join(REPO_ROOT, "target/pi-sidecar/pi")),
		piJsBinary: argValue("--pi-js-binary", "pi"),
		piExtension,
		codexBinary: argValue("--codex-binary", "codex"),
		expectedPatterns,
	};
}

function dryRunReceipt(
	path: BenchmarkPath,
	trial: number,
	query: string,
	command: { executable: string; args: string[] },
): RunReceipt {
	return {
		type: "agent-chat.web-search-benchmark.run.v1",
		path,
		trial,
		query,
		status: "dry-run",
		startedAt: new Date().toISOString(),
		command: redactCommand(command),
		timingsMs: {
			processSpawn: 0,
			firstEvent: null,
			firstWebSearch: null,
			firstAnswer: null,
			total: 0,
		},
		webSearchObserved: false,
		usefulAnswerObserved: false,
		sourceUrlCount: 0,
		expectedPatternsMatched: false,
		answer: "",
		answerChars: 0,
		exitCode: null,
		error: null,
	};
}

export async function main() {
	const options = parseOptions();
	const contract = loadQuickAiContract();
	if (contract.tools.join(",") !== "web_search") {
		throw new Error(
			`Quick AI tools changed to ${contract.tools.join(",")}; benchmark must be reviewed`,
		);
	}
	if (
		contract.focusedSearchBudget !== 1 ||
		!contract.appendSystemPrompt.includes(
			"exactly one web_search action containing one focused query",
		)
	) {
		throw new Error(
			"Quick AI search budget and system prompt must stay aligned at one focused search",
		);
	}

	const cwd = resolve(
		process.env.TMPDIR ?? "/tmp",
		"script-kit-agent-chat-web-search-benchmark",
	);
	mkdirSync(cwd, { recursive: true });
	const runs: RunReceipt[] = [];

	for (const path of options.paths) {
		const piCommand = piRpcCommand(contract, options.piBinary);
		const warm =
			!options.dryRun && path === "pi-rpc-warm"
				? await startPersistentPi(piCommand, cwd, options.timeoutMs)
				: undefined;
		for (let trial = 1; trial <= options.trials; trial++) {
			const query = options.queries[(trial - 1) % options.queries.length];
			const command =
				path === "codex-exec"
					? codexExecCommand(contract, options.codexBinary, cwd, query)
					: path === "pi-extension-cold"
						? piExtensionCommand(
								contract,
								options.piJsBinary,
								options.piExtension,
								query,
							)
						: piRpcCommand(contract, options.piBinary);
			if (options.dryRun) {
				runs.push(dryRunReceipt(path, trial, query, command));
				continue;
			}

			const live = await runProcess(
				path,
				trial,
				query,
				command,
				cwd,
				options.timeoutMs,
				path === "codex-exec"
					? parseCodexEvent
					: path === "pi-extension-cold"
						? parsePiJsonEvent
						: parsePiRpcEvent,
				path === "pi-rpc-cold" || path === "pi-rpc-warm",
				options.expectedPatterns,
				path === "pi-rpc-warm" ? warm : undefined,
			);
			runs.push(live.receipt);
			process.stdout.write(`${JSON.stringify(live.receipt)}\n`);
		}
		if (warm) {
			warm.process.kill();
			await warm.process.exited;
			await Promise.all([warm.stdout, warm.stderr]);
		}
	}

	const ranking = summarizeRuns(runs);
	const receipt = {
		type: "agent-chat.web-search-benchmark.v1",
		generatedAt: new Date().toISOString(),
		profile: contract,
		qualityCriteria: {
			expectedPatterns: options.expectedPatterns,
			requiresSourceUrl: true,
			rejectsKnownEmptyResultLanguage: true,
		},
		timingSemantics: {
			total: "process spawn or warm prompt write through terminal turn event",
			firstWebSearch: "first native event that identifies web_search",
			firstAnswer:
				"first native assistant text event; Codex JSONL may expose only the completed agent message",
			validRun:
				"transport succeeded, native web_search was observed, and the final answer contains a source URL without a known empty-result/failure marker",
		},
		runs,
		...ranking,
	};
	if (options.output) {
		const output = resolve(options.output);
		mkdirSync(dirname(output), { recursive: true });
		writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
	}
	process.stdout.write(`${JSON.stringify(receipt)}\n`);
}

if (import.meta.main) {
	await main();
}

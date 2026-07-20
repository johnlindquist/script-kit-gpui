#!/usr/bin/env bun
/// <reference types="bun-types" />
/**
 * NN=30 Terminal prompt / PTY chaos battery.
 *
 * PREP contract: this file may be transpiled but MUST NOT be launched until a
 * manager names the lane for a runtime slot. Runtime is hidden/protocol-first,
 * sandbox-HOME only, and executes only the inert fixtures seeded below.
 */

import {
	chmodSync,
	existsSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver.ts";

const PROJECT_ROOT = resolve(import.meta.dir, "../..");
const RUN_ID = new Date().toISOString().replace(/[:.]/g, "-");
const LANE = process.env.NN30_LANE?.trim() || "finder-terminal";
const OUTPUT_DIR = resolve(
	PROJECT_ROOT,
	process.env.NN30_RECEIPT_DIR ??
		join(".test-output/chaos-30-terminal", `${LANE}-${RUN_ID}`),
);
const BINARY = resolve(
	PROJECT_ROOT,
	process.env.NN30_BINARY ??
		process.env.SCRIPT_KIT_GPUI_BINARY ??
		"target-agent/artifacts/finder-terminal/script-kit-gpui",
);
const MAIN_TARGET: Json = { type: "kind", kind: "main" };
const ACTIONS_TARGET: Json = { type: "kind", kind: "actionsDialog" };
const VIEW = "quickTerminal";
const ALL_ROWS = [
	"spawn-exit-codes",
	"cwd-env-inheritance",
	"resize-grid-stability",
	"ansi-osc-hostile",
	"huge-output-flood",
	"ctrl-c-kill",
	"command-bar-interactions",
	"theme-hot-reload",
] as const;
type RowId = (typeof ALL_ROWS)[number];
type CheckKind = "product" | "harness" | "environment";
type Check = { name: string; ok: boolean; kind: CheckKind; detail: Json };
type RowReceipt = {
	schemaVersion: 1;
	rowId: RowId;
	classification: string;
	checks: Check[];
	productFindings: Json[];
	harnessFindings: Json[];
	environmentFindings: Json[];
	evidence: Json;
	startedAt: string;
	finishedAt?: string;
};

type FixtureName =
	| "exit-code.sh"
	| "env-cwd.sh"
	| "stty-size.sh"
	| "ansi-osc.sh"
	| "flood.sh"
	| "signal.sh";

const FIXTURES: Record<FixtureName, string> = {
	"exit-code.sh": `#!/bin/sh
code="\${1:-0}"
printf 'NN30-EXIT-FIXTURE code=%s\\n' "$code"
exit "$code"
`,
	"env-cwd.sh": `#!/bin/sh
expected_home="$1"
expected_cwd="$2"
[ "$HOME" = "$expected_home" ] && home_ok=1 || home_ok=0
[ "$PWD" = "$expected_cwd" ] && cwd_ok=1 || cwd_ok=0
printf 'NN30-HOME-OK=%s\\n' "$home_ok"
printf 'NN30-CWD-OK=%s\\n' "$cwd_ok"
printf 'NN30-TERM=%s\\n' "$TERM"
printf 'NN30-COLORTERM=%s\\n' "$COLORTERM"
printf 'NN30-TERM-PROGRAM=%s\\n' "\${TERM_PROGRAM-}"
printf 'NN30-HOST-ONLY=%s\\n' "\${NN30_HOST_ONLY_SECRET-}"
`,
	"stty-size.sh": `#!/bin/sh
tag="\${1:-default}"
set -- $(stty size)
printf 'NN30-STTY-%s rows=%s cols=%s\\n' "$tag" "$1" "$2"
`,
	"ansi-osc.sh": `#!/bin/sh
printf '\\033]0;NN30-OSC-TITLE\\007'
printf '\\033[31mNN30-SGR-RED\\033[0m\\n'
printf '\\033]8;;https://example.invalid/nn30\\007NN30-LINK\\033]8;;\\007\\n'
printf '\\033[999999999CNN30-INVALID-CSI\\n'
printf '\\033]0;'
awk 'BEGIN { for (i=0; i<4096; i++) printf "T" }'
printf '\\007NN30-LONG-TITLE-DONE\\n'
`,
	"flood.sh": `#!/bin/sh
lines="\${1:-100000}"
awk -v n="$lines" 'BEGIN {
  for (i=1; i<=n; i++) printf "NN30-FLOOD-%06d abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\\n", i;
  printf "NN30-FLOOD-DONE lines=%d\\n", n;
}'
`,
	"signal.sh": `#!/bin/sh
tag="\${1:-untagged}"
trap 'printf "NN30-SIGNAL-%s=INT\\n" "$tag"; exit 130' INT
trap 'printf "NN30-SIGNAL-%s=TERM\\n" "$tag"; exit 143' TERM
trap 'printf "NN30-SIGNAL-%s=HUP\\n" "$tag"; exit 129' HUP
printf 'NN30-SIGNAL-%s-READY pid=%s\\n' "$tag" "$$"
while :; do sleep 1; done
`,
};

class HarnessInvalid extends Error {}
class EnvironmentBlocked extends Error {}

function selectedRows(): RowId[] {
	const index = process.argv.indexOf("--rows");
	if (index < 0) return [...ALL_ROWS];
	const requested = [
		...new Set(
			String(process.argv[index + 1] ?? "")
				.split(",")
				.map((row) => row.trim())
				.filter(Boolean),
		),
	];
	const unknown = requested.filter((row) => !ALL_ROWS.includes(row as RowId));
	if (unknown.length > 0)
		throw new HarnessInvalid(`unknown --rows values: ${unknown.join(", ")}`);
	if (requested.length === 0)
		throw new HarnessInvalid("--rows selected no rows");
	return requested as RowId[];
}

function classifyLaunchError(error: unknown): never {
	const message = error instanceof Error ? error.message : String(error);
	if (
		/did not become ready|rpc.*timeout|timed out|operation not permitted|sandbox/i.test(
			message,
		)
	)
		throw new EnvironmentBlocked(message);
	throw new HarnessInvalid(message);
}

async function poll<T>(
	label: string,
	probe: () => Promise<T>,
	accept: (value: T) => boolean,
	timeoutMs = 8_000,
	intervalMs = 40,
): Promise<T> {
	const started = performance.now();
	let last: T | undefined;
	let lastError: unknown;
	while (performance.now() - started < timeoutMs) {
		try {
			last = await probe();
			if (accept(last)) return last;
		} catch (error) {
			lastError = error;
		}
		await Bun.sleep(intervalMs);
	}
	throw new HarnessInvalid(
		`${label} not observable in ${timeoutMs}ms; last=${JSON.stringify(last)} error=${String(lastError ?? "")}`,
	);
}

function shellQuote(value: string): string {
	return `'${value.replaceAll("'", `'"'"'`)}'`;
}

function walk(node: unknown, out: Json[] = []): Json[] {
	if (!node || typeof node !== "object") return out;
	if (Array.isArray(node)) {
		for (const child of node) walk(child, out);
		return out;
	}
	const json = node as Json;
	out.push(json);
	for (const value of Object.values(json)) walk(value, out);
	return out;
}

function renderedText(elements: Json): string {
	return walk(elements)
		.flatMap((node) =>
			["title", "label", "text", "value"].map((key) => node[key]),
		)
		.filter((value): value is string => typeof value === "string")
		.join("\n");
}

async function writeJson(path: string, value: unknown): Promise<void> {
	await Bun.write(path, `${JSON.stringify(value, null, 2)}\n`);
}

function seedFixtures(sessionDir: string): {
	dir: string;
	paths: Record<FixtureName, string>;
	manifest: Json;
} {
	const dir = join(
		sessionDir,
		"home",
		".scriptkit",
		"chaos-30-terminal-fixtures",
	);
	mkdirSync(dir, { recursive: true });
	const paths = {} as Record<FixtureName, string>;
	const manifest: Json[] = [];
	for (const [name, contents] of Object.entries(FIXTURES) as [
		FixtureName,
		string,
	][]) {
		const path = join(dir, name);
		writeFileSync(path, contents, { encoding: "utf8", mode: 0o700 });
		chmodSync(path, 0o700);
		paths[name] = path;
		manifest.push({
			name,
			path,
			bytes: Buffer.byteLength(contents),
			mode: "0700",
		});
	}
	return { dir, paths, manifest };
}

async function runBattery(): Promise<void> {
	const rows = selectedRows();
	if (existsSync(OUTPUT_DIR) && readdirSync(OUTPUT_DIR).length > 0) {
		throw new HarnessInvalid(
			`receipt directory already contains files: ${OUTPUT_DIR}`,
		);
	}
	mkdirSync(OUTPUT_DIR, { recursive: true });
	const summary: Json = {
		schemaVersion: 1,
		tool: "chaos-30-terminal-probe",
		nn: 30,
		runId: RUN_ID,
		lane: LANE,
		binary: BINARY,
		outputDir: OUTPUT_DIR,
		executor: "Driver.launch sandboxHome hidden/protocol-first",
		screen: {
			claimed: false,
			shown: false,
			screenshots: false,
			nativeInput: false,
		},
		requestedRows: rows,
		rowReceipts: [],
		productFindings: [],
		harnessFindings: [],
		environmentFindings: [],
	};

	console.error(`[driver] binary: ${BINARY} (explicit NN=30 pin)`);
	let driver: Driver;
	try {
		driver = await Driver.launch({
			binary: BINARY,
			sandboxHome: true,
			seedAgentAuth: false,
			sharedModels: false,
			sessionName: `chaos-30-terminal-${LANE}-${RUN_ID}`,
			readyTimeoutMs: 30_000,
			defaultTimeoutMs: 10_000,
			env: { NN30_HOST_ONLY_SECRET: "must-not-reach-pty" },
		});
	} catch (error) {
		try {
			classifyLaunchError(error);
		} catch (classified) {
			const message =
				classified instanceof Error ? classified.message : String(classified);
			const finding = { phase: "launch", message };
			if (classified instanceof EnvironmentBlocked)
				summary.environmentFindings.push(finding);
			else summary.harnessFindings.push(finding);
			summary.classification =
				classified instanceof EnvironmentBlocked
					? "blocked-by-environment"
					: "invalid-harness";
			summary.pass = false;
			await writeJson(join(OUTPUT_DIR, "summary.json"), summary);
			console.log(JSON.stringify(summary, null, 2));
			process.exitCode = classified instanceof EnvironmentBlocked ? 3 : 2;
			return;
		}
	}

	const fixtures = seedFixtures(driver.sessionDir);
	await writeJson(join(OUTPUT_DIR, "fixture-manifest.json"), fixtures.manifest);

	async function diagnosticCall(
		label: string,
		call: () => Promise<unknown>,
	): Promise<Json> {
		try {
			return (await call()) as Json;
		} catch (error) {
			return {
				label,
				error: error instanceof Error ? error.message : String(error),
			};
		}
	}

	async function state(): Promise<Json> {
		return (await driver.getState({ timeoutMs: 6_000 })) as Json;
	}

	async function windows(): Promise<Json> {
		return (await driver.listAutomationWindows({ timeoutMs: 6_000 })) as Json;
	}

	async function terminalElements(): Promise<Json> {
		return (await driver.getElements(
			{ target: MAIN_TARGET, limit: 500 },
			{ timeoutMs: 8_000 },
		)) as Json;
	}

	async function terminalText(): Promise<string> {
		return renderedText(await terminalElements());
	}

	async function waitForText(
		marker: string,
		timeoutMs = 8_000,
	): Promise<string> {
		return poll(
			"terminal text marker",
			terminalText,
			(text) => text.includes(marker),
			timeoutMs,
		);
	}

	async function typeRaw(text: string): Promise<Json> {
		const result = (await driver.batch([{ type: "setInput", text }], {
			stopOnError: true,
			timeoutMs: 8_000,
		})) as Json;
		if (result.success !== true)
			throw new HarnessInvalid(
				`terminal batch setInput failed: ${JSON.stringify(result)}`,
			);
		return result;
	}

	async function runFixture(
		name: FixtureName,
		args: string[] = [],
	): Promise<Json> {
		const command = [fixtures.paths[name], ...args].map(shellQuote).join(" ");
		return typeRaw(`${command}\r`);
	}

	let ptyGeneration = 0;
	async function openTerminal(label: string): Promise<Json> {
		const generation = ++ptyGeneration;
		driver.send({
			type: "triggerBuiltin",
			builtinId: "builtin/main-window",
			requestId: `${RUN_ID}-${label}-${generation}-reset-terminal`,
		});
		const reset = await poll(
			`${label} reset stale terminal`,
			state,
			(value) => value.promptType !== VIEW,
			10_000,
		);
		await driver.waitForSettle({ timeoutMs: 5_000 }).catch(() => {});
		driver.send({
			type: "triggerBuiltin",
			builtinId: "builtin/quick-terminal",
			requestId: `${RUN_ID}-${label}-${generation}-spawn-terminal`,
		});
		const opened = await poll(
			`${label} fresh Quick Terminal open`,
			state,
			(value) => value.promptType === VIEW,
			10_000,
		);
		await driver.waitForSettle({ timeoutMs: 5_000 }).catch(() => {});

		const nonce = `${RUN_ID}-${label}-${generation}`;
		const marker = `NN30-PTY-LIVE-${nonce}`;
		const command = `printf 'NN30-PTY-LIVE-%s\\n' ${shellQuote(nonce)}`;
		if (command.includes(marker)) {
			throw new HarnessInvalid(
				`${label} PTY liveness command contains its expected marker`,
			);
		}
		const beforeText = await terminalText();
		if (beforeText.includes(marker)) {
			throw new HarnessInvalid(
				`${label} PTY marker was stale before execution`,
			);
		}
		await typeRaw(`${command}\r`);
		const executedText = await waitForText(marker, 8_000);
		const ptyProof: Json = {
			label,
			generation,
			resetPromptType: reset.promptType ?? null,
			openedPromptType: opened.promptType ?? null,
			marker,
			command,
			commandContainsMarker: command.includes(marker),
			executedMarkerObserved: executedText.includes(marker),
		};
		return { ...opened, ptyProof };
	}

	async function dialogState(): Promise<Json | null> {
		try {
			const result = (await driver.request(
				{ type: "getState", target: ACTIONS_TARGET, summaryOnly: true },
				{ expect: "stateResult", timeoutMs: 5_000 },
			)) as Json;
			return (result.actionsDialog ?? result) as Json;
		} catch {
			return null;
		}
	}

	async function actionsWindows(): Promise<Json[]> {
		const table = await windows();
		return ((table.windows ?? []) as Json[]).filter((window) =>
			String(window.kind ?? window.id ?? window.automationId ?? "")
				.toLowerCase()
				.includes("action"),
		);
	}

	async function gpuiKey(
		key: string,
		modifiers: string[] = [],
		target: Json = MAIN_TARGET,
	): Promise<Json> {
		return driver.simulateGpuiEvent(
			{ type: "keyDown", key, modifiers },
			{ target, timeoutMs: 8_000 },
		);
	}

	async function openTerminalActions(): Promise<Json> {
		await gpuiKey("k", ["cmd"]);
		return poll(
			"terminal actions popup",
			async () => ({
				state: await dialogState(),
				windows: await actionsWindows(),
			}),
			(value) => Boolean(value.state) && value.windows.length === 1,
			8_000,
		);
	}

	async function activateTerminalAction(
		actionId: string,
		title: string,
	): Promise<Json> {
		await openTerminalActions();
		const filterResult = (await driver.request(
			{
				type: "batch",
				target: ACTIONS_TARGET,
				commands: [{ type: "setInput", text: title }],
				options: { stopOnError: true, timeout: 6_000 },
			},
			{ expect: "batchResult", timeoutMs: 8_000 },
		)) as Json;
		if (filterResult.success !== true)
			throw new HarnessInvalid(
				`filter terminal action failed: ${JSON.stringify(filterResult)}`,
			);
		const elements = (await driver.getElements(
			{ target: ACTIONS_TARGET, limit: 300 },
			{ timeoutMs: 6_000 },
		)) as Json;
		const node = walk(elements).find((candidate) => {
			const semanticId = String(candidate.semanticId ?? candidate.id ?? "");
			const label = String(
				candidate.label ?? candidate.title ?? candidate.text ?? "",
			);
			return semanticId.endsWith(`:${actionId}`) || label === title;
		});
		const semanticId = String(node?.semanticId ?? node?.id ?? "");
		if (!semanticId)
			throw new HarnessInvalid(
				`terminal action ${actionId}/${title} lacked semantic id`,
			);
		const selection = (await driver.request(
			{
				type: "batch",
				target: ACTIONS_TARGET,
				commands: [{ type: "selectBySemanticId", semanticId }],
				options: { stopOnError: true, timeout: 6_000 },
			},
			{ expect: "batchResult", timeoutMs: 8_000 },
		)) as Json;
		if (selection.success !== true)
			throw new HarnessInvalid(
				`select terminal action failed: ${JSON.stringify(selection)}`,
			);
		const dispatch = await gpuiKey("enter", [], ACTIONS_TARGET);
		await poll(
			`terminal action ${actionId} closes popup`,
			actionsWindows,
			(value) => value.length === 0,
			8_000,
		);
		return { filterResult, semanticId, selection, dispatch };
	}

	async function baselineErrors(): Promise<Set<string>> {
		const result = (await driver
			.getLogs({ level: "error", limit: 500 }, { timeoutMs: 5_000 })
			.catch(() => ({ entries: [] }))) as Json;
		const entries = (result.entries ?? result.logs ?? []) as Json[];
		return new Set(
			entries.map((entry) => `${entry.target ?? ""}|${entry.message ?? ""}`),
		);
	}

	async function errorDelta(before: Set<string>): Promise<string[]> {
		const after = await baselineErrors();
		return [...after].filter((entry) => !before.has(entry));
	}

	async function executeRow(
		rowId: RowId,
		body: (
			receipt: RowReceipt,
			check: (
				name: string,
				ok: boolean,
				kind?: CheckKind,
				detail?: Json,
			) => void,
			captureSample: (label: string) => Promise<void>,
			openFreshTerminal: (label: string) => Promise<Json>,
		) => Promise<void>,
	): Promise<void> {
		const startedAtMs = Date.now();
		const rowDir = join(OUTPUT_DIR, rowId);
		mkdirSync(rowDir, { recursive: true });
		const receipt: RowReceipt = {
			schemaVersion: 1,
			rowId,
			classification: "running",
			checks: [],
			productFindings: [],
			harnessFindings: [],
			environmentFindings: [],
			evidence: {},
			startedAt: new Date(startedAtMs).toISOString(),
		};
		const stateSamples: Json[] = [];
		const windowSamples: Json[] = [];
		const captureSample = async (label: string) => {
			const capturedAt = new Date().toISOString();
			const [main, windowTable] = await Promise.all([
				diagnosticCall(`${label}:state`, state),
				diagnosticCall(`${label}:windows`, windows),
			]);
			stateSamples.push({ label, capturedAt, main });
			windowSamples.push({ label, capturedAt, windowTable });
		};
		const check = (
			name: string,
			ok: boolean,
			kind: CheckKind = "product",
			detail: Json = {},
		) => {
			const entry = { name, ok, kind, detail };
			receipt.checks.push(entry);
			if (!ok) {
				const finding = { name, detail };
				if (kind === "product") receipt.productFindings.push(finding);
				else if (kind === "environment")
					receipt.environmentFindings.push(finding);
				else receipt.harnessFindings.push(finding);
			}
		};
		const ptyLiveness: Json[] = [];
		const openFreshTerminal = async (label: string): Promise<Json> => {
			const opened = await openTerminal(label);
			ptyLiveness.push(opened.ptyProof as Json);
			return opened;
		};
		const beforeErrors = await baselineErrors();
		try {
			const opened = await openTerminal(rowId);
			ptyLiveness.push(opened.ptyProof as Json);
			await captureSample("before-row");
			await body(receipt, check, captureSample, openFreshTerminal);
			const freshErrors = await errorDelta(beforeErrors);
			check("no_new_error_logs", freshErrors.length === 0, "product", {
				freshErrors,
			});
		} catch (error) {
			const message = error instanceof Error ? error.message : String(error);
			if (error instanceof EnvironmentBlocked)
				receipt.environmentFindings.push({ message });
			else
				receipt.harnessFindings.push({
					message,
					unclassifiedException: !(error instanceof HarnessInvalid),
				});
		}

		await captureSample("after-row");
		const capturedAt = new Date().toISOString();
		const [layout, elements, logs] = await Promise.all([
			diagnosticCall("layout", () =>
				driver.getLayoutInfo({ target: MAIN_TARGET }, { timeoutMs: 8_000 }),
			),
			diagnosticCall("elements", terminalElements),
			diagnosticCall("logs", () =>
				driver.getLogs({ limit: 500 }, { timeoutMs: 8_000 }),
			),
		]);
		const finishedAtMs = Date.now();
		const timings = {
			rowId,
			startedAtMs,
			finishedAtMs,
			durationMs: finishedAtMs - startedAtMs,
			capturedAt,
		};
		await Promise.all([
			writeJson(join(rowDir, "layout.json"), layout),
			writeJson(join(rowDir, "elements.json"), elements),
			writeJson(join(rowDir, "app-logs.json"), logs),
			writeJson(join(rowDir, "state-samples.json"), stateSamples),
			writeJson(join(rowDir, "windows.json"), windowSamples),
			writeJson(join(rowDir, "timings.json"), timings),
		]);
		receipt.evidence = {
			...receipt.evidence,
			ptyLiveness,
			diagnosticBundle: {
				rowDir,
				layout: "layout.json",
				elements: "elements.json",
				logs: "app-logs.json",
				stateSamples: "state-samples.json",
				windows: "windows.json",
				timings: "timings.json",
			},
		};
		receipt.classification = receipt.environmentFindings.length
			? "blocked-by-environment"
			: receipt.harnessFindings.length
				? "invalid-harness"
				: receipt.productFindings.length
					? "failed-product"
					: "verified";
		receipt.finishedAt = new Date(finishedAtMs).toISOString();
		await Promise.all([
			writeJson(join(rowDir, "receipt.json"), receipt),
			writeJson(join(OUTPUT_DIR, `${rowId}.json`), receipt),
		]);
		summary.rowReceipts.push({ rowId, classification: receipt.classification });
		summary.productFindings.push(
			...receipt.productFindings.map((finding) => ({ rowId, ...finding })),
		);
		summary.harnessFindings.push(
			...receipt.harnessFindings.map((finding) => ({ rowId, ...finding })),
		);
		summary.environmentFindings.push(
			...receipt.environmentFindings.map((finding) => ({ rowId, ...finding })),
		);
	}

	const rowBodies: Record<
		RowId,
		(
			receipt: RowReceipt,
			check: (
				name: string,
				ok: boolean,
				kind?: CheckKind,
				detail?: Json,
			) => void,
			captureSample: (label: string) => Promise<void>,
			openFreshTerminal: (label: string) => Promise<Json>,
		) => Promise<void>
	> = {
		"spawn-exit-codes": async (
			receipt,
			check,
			captureSample,
			openFreshTerminal,
		) => {
			await runFixture("exit-code.sh", ["7"]);
			await typeRaw("printf 'NN30-STATUS=%s\\n' \"$?\"\r");
			const statusText = await waitForText("NN30-STATUS=7", 8_000);
			check(
				"subprocess_exit_status_7_preserved",
				statusText.includes("NN30-STATUS=7"),
				"product",
				{
					marker: "NN30-STATUS=7",
				},
			);
			await captureSample("after-subprocess-exit-7");

			await typeRaw("printf 'NN30-SHELL-EXIT=23\\n'; exit 23\r");
			await waitForText("NN30-SHELL-EXIT=23", 5_000);
			const exitLog = await poll(
				"terminal shell exit app log",
				async () => readFileSync(driver.logPath, "utf8"),
				(log) => /Terminal (?:process )?exited/.test(log),
				8_000,
			);
			const exactExitCode =
				/code=23[^\n]*Terminal exited/.test(exitLog) ||
				/Terminal exited[^\n]*(?:code=23|code\s*:\s*23)/.test(exitLog);
			check("shell_exit_code_23_preserved", exactExitCode, "product", {
				expectedCode: 23,
				matchingLines: exitLog
					.split("\n")
					.filter((line) => /Terminal (?:process )?exited/.test(line))
					.slice(-6),
			});
			await captureSample("after-shell-exit-23");
			const appAlive = driver.alive;
			const reopened = await openFreshTerminal("spawn-exit-codes-post-exit");
			const postExitPtyProof = (reopened.ptyProof ?? {}) as Json;
			check(
				"shell_exit_23_does_not_exit_app",
				appAlive &&
					reopened.promptType === VIEW &&
					postExitPtyProof.executedMarkerObserved === true,
				"product",
				{
					appAlive,
					reopenedPromptType: reopened.promptType ?? null,
					postExitPtyProof,
				},
			);
			receipt.evidence = {
				exitFixture: fixtures.paths["exit-code.sh"],
				subprocessMarker: "NN30-STATUS=7",
				shellExitMarker: "NN30-SHELL-EXIT=23",
				postExitPtyProof,
				appLog: driver.logPath,
			};
		},
		"cwd-env-inheritance": async (receipt, check, captureSample) => {
			const sandboxHome = join(driver.sessionDir, "home");
			await typeRaw(`cd ${shellQuote(fixtures.dir)}\r`);
			await runFixture("env-cwd.sh", [sandboxHome, fixtures.dir]);
			const text = await waitForText("NN30-HOST-ONLY=", 8_000);
			const expectedMarkers = [
				"NN30-HOME-OK=1",
				"NN30-CWD-OK=1",
				"NN30-TERM=xterm-256color",
				"NN30-COLORTERM=truecolor",
				"NN30-TERM-PROGRAM=",
				"NN30-HOST-ONLY=",
			];
			const missing = expectedMarkers.filter(
				(marker) => !text.includes(marker),
			);
			check(
				"pty_env_allowlist_and_cwd_are_exact",
				missing.length === 0,
				"product",
				{
					expectedMarkers,
					missing,
				},
			);
			check(
				"host_only_secret_is_scrubbed",
				!text.includes("must-not-reach-pty"),
				"product",
				{
					secretMarkerObserved: text.includes("must-not-reach-pty"),
				},
			);
			await captureSample("after-env-cwd-fixture");
			await typeRaw(`cd ${shellQuote(PROJECT_ROOT)}\r`);
			receipt.evidence = {
				fixture: fixtures.paths["env-cwd.sh"],
				sandboxHome,
				fixtureDir: fixtures.dir,
				expectedMarkers,
			};
		},
		"resize-grid-stability": async (receipt, check, captureSample) => {
			await runFixture("stty-size.sh");
			const text = await waitForText("NN30-STTY-default", 8_000);
			const sizeMatch = text.match(/NN30-STTY-default rows=(\d+) cols=(\d+)/);
			const rows = Number(sizeMatch?.[1] ?? 0);
			const cols = Number(sizeMatch?.[2] ?? 0);
			check(
				"pty_reports_nonzero_rendered_grid",
				rows >= 2 && cols >= 10,
				"product",
				{
					rows,
					cols,
					markerObserved: Boolean(sizeMatch),
				},
			);

			const frames: Json[] = [];
			for (let index = 0; index < 6; index += 1) {
				const layout = (await driver.getLayoutInfo(
					{ target: MAIN_TARGET },
					{ timeoutMs: 8_000 },
				)) as Json;
				frames.push({
					index,
					capturedAt: new Date().toISOString(),
					windowBounds: layout.windowBounds ?? layout.window ?? null,
					components: layout.components ?? [],
				});
				await Bun.sleep(50);
			}
			const frameFingerprints = frames.map((frame) =>
				JSON.stringify({
					windowBounds: frame.windowBounds,
					components: frame.components,
				}),
			);
			check(
				"settled_terminal_layout_has_no_resize_jitter",
				new Set(frameFingerprints).size === 1,
				"product",
				{
					uniqueFrames: new Set(frameFingerprints).size,
					frameCount: frames.length,
				},
			);
			await captureSample("after-resize-stability-sampling");
			receipt.evidence = {
				fixture: fixtures.paths["stty-size.sh"],
				ptySize: { rows, cols },
				frames,
				note: "No protocol setWindowBounds primitive exists; this row proves initial PTY resize plus post-settle grid/layout stability.",
			};
		},
		"ansi-osc-hostile": async (receipt, check, captureSample) => {
			const beforeWindows = await windows();
			await runFixture("ansi-osc.sh");
			const text = await waitForText("NN30-LONG-TITLE-DONE", 10_000);
			const visibleMarkers = [
				"NN30-SGR-RED",
				"NN30-LINK",
				"NN30-INVALID-CSI",
				"NN30-LONG-TITLE-DONE",
			];
			const missing = visibleMarkers.filter((marker) => !text.includes(marker));
			check(
				"ansi_osc_payload_renders_visible_text",
				missing.length === 0,
				"product",
				{
					visibleMarkers,
					missing,
				},
			);
			const leakedControls = [...text].filter(
				(char) => char === "\u001b" || char === "\u0007" || char === "\u0000",
			);
			check(
				"ansi_osc_controls_do_not_leak_into_semantics",
				leakedControls.length === 0,
				"product",
				{
					leakedControlCount: leakedControls.length,
				},
			);
			const afterWindows = await windows();
			const ids = (table: Json) =>
				((table.windows ?? []) as Json[])
					.map(
						(window) =>
							`${window.kind ?? ""}:${window.id ?? window.automationId ?? ""}`,
					)
					.sort((a, b) => a.localeCompare(b));
			check(
				"osc_title_and_hyperlink_create_no_extra_windows",
				JSON.stringify(ids(beforeWindows)) ===
					JSON.stringify(ids(afterWindows)),
				"product",
				{ before: ids(beforeWindows), after: ids(afterWindows) },
			);
			const finalState = await state();
			check(
				"hostile_sequences_keep_terminal_alive",
				driver.alive && finalState.promptType === VIEW,
				"product",
				{
					appAlive: driver.alive,
					promptType: finalState.promptType ?? null,
				},
			);
			await captureSample("after-ansi-osc-hostile");
			receipt.evidence = {
				fixture: fixtures.paths["ansi-osc.sh"],
				visibleMarkers,
				beforeWindowIds: ids(beforeWindows),
				afterWindowIds: ids(afterWindows),
			};
		},
		"huge-output-flood": async (receipt, check, captureSample) => {
			const lineCount = 100_000;
			const started = performance.now();
			await runFixture("flood.sh", [String(lineCount)]);
			const samples: Json[] = [];
			let sawTail = false;
			for (let index = 0; index < 160; index += 1) {
				const rpcStarted = performance.now();
				const current = await state();
				const stateLatencyMs = performance.now() - rpcStarted;
				let tailObserved = false;
				if (index % 4 === 0) {
					const text = await terminalText();
					tailObserved = text.includes(`NN30-FLOOD-DONE lines=${lineCount}`);
					sawTail ||= tailObserved;
				}
				samples.push({
					index,
					capturedAt: new Date().toISOString(),
					elapsedMs: Math.round(performance.now() - started),
					stateLatencyMs: Number(stateLatencyMs.toFixed(2)),
					promptType: current.promptType ?? null,
					tailObserved,
				});
				if (sawTail) break;
				await Bun.sleep(50);
			}
			const durationMs = Math.round(performance.now() - started);
			const latencies = samples.map((sample) => Number(sample.stateLatencyMs));
			const sorted = [...latencies].sort((a, b) => a - b);
			const p95Ms = sorted.length
				? sorted[
						Math.min(sorted.length - 1, Math.ceil(sorted.length * 0.95) - 1)
					]
				: Number.POSITIVE_INFINITY;
			check(
				"flood_tail_reaches_terminal",
				sawTail && durationMs <= 15_000,
				"product",
				{
					sawTail,
					durationMs,
					budgetMs: 15_000,
					lineCount,
				},
			);
			check(
				"state_rpc_stays_responsive_during_flood",
				p95Ms <= 250,
				"product",
				{
					p95Ms,
					budgetMs: 250,
					sampleCount: samples.length,
				},
			);
			const finalState = await state();
			check(
				"flood_keeps_terminal_surface_alive",
				driver.alive && finalState.promptType === VIEW,
				"product",
				{
					appAlive: driver.alive,
					promptType: finalState.promptType ?? null,
				},
			);
			await captureSample("after-flood-tail");
			receipt.evidence = {
				fixture: fixtures.paths["flood.sh"],
				lineCount,
				durationMs,
				stateRpcP95Ms: p95Ms,
				samples,
			};
		},
		"ctrl-c-kill": async (receipt, check, captureSample) => {
			await runFixture("signal.sh", ["raw"]);
			await waitForText("NN30-SIGNAL-raw-READY", 8_000);
			await typeRaw("\u0003");
			const afterInterrupt = await waitForText("NN30-SIGNAL-raw=INT", 8_000);
			check(
				"raw_ctrl_c_delivers_sigint",
				afterInterrupt.includes("NN30-SIGNAL-raw=INT"),
				"product",
				{
					marker: "NN30-SIGNAL-raw=INT",
				},
			);
			await captureSample("after-raw-ctrl-c");

			await runFixture("signal.sh", ["kill-action"]);
			await waitForText("NN30-SIGNAL-kill-action-READY", 8_000);
			await openTerminalActions();
			const killElements = (await driver.getElements(
				{ target: ACTIONS_TARGET, limit: 400 },
				{ timeoutMs: 8_000 },
			)) as Json;
			const killReachable = walk(killElements).some((node) => {
				const semantic = String(node.semanticId ?? node.id ?? "");
				const label = String(node.label ?? node.title ?? node.text ?? "");
				return semantic.endsWith(":kill") || label === "Kill Process";
			});
			check(
				"kill_process_is_reachable_from_terminal_command_bar",
				killReachable,
				"product",
				{
					documentedAction: "Kill Process — Send SIGTERM to terminate process",
				},
			);
			await gpuiKey("k", ["cmd"], MAIN_TARGET);
			await poll(
				"close kill reachability popup",
				actionsWindows,
				(value) => value.length === 0,
				8_000,
			);

			let killAction: Json | null = null;
			let sawTerm = false;
			let sawInt = false;
			if (killReachable) {
				killAction = await activateTerminalAction("kill", "Kill Process");
				const afterKill = await poll(
					"Kill Process signal marker",
					terminalText,
					(text) =>
						text.includes("NN30-SIGNAL-kill-action=TERM") ||
						text.includes("NN30-SIGNAL-kill-action=INT"),
					8_000,
				);
				sawTerm = afterKill.includes("NN30-SIGNAL-kill-action=TERM");
				sawInt = afterKill.includes("NN30-SIGNAL-kill-action=INT");
				check(
					"kill_process_delivers_documented_sigterm",
					sawTerm && !sawInt,
					"product",
					{
						documentedSignal: "SIGTERM",
						sawTerm,
						sawInt,
					},
				);
			} else {
				await typeRaw("\u0003");
				await waitForText("NN30-SIGNAL-kill-action=INT", 8_000);
			}
			await typeRaw("printf 'NN30-AFTER-SIGNALS-ALIVE\\n'\r");
			const finalText = await waitForText("NN30-AFTER-SIGNALS-ALIVE", 8_000);
			check(
				"shell_remains_usable_after_signal_actions",
				finalText.includes("NN30-AFTER-SIGNALS-ALIVE"),
				"product",
				{
					marker: "NN30-AFTER-SIGNALS-ALIVE",
				},
			);
			await captureSample("after-command-bar-kill");
			receipt.evidence = {
				fixture: fixtures.paths["signal.sh"],
				rawInterruptMarker: "NN30-SIGNAL-raw=INT",
				killReachable,
				killMarkers: { term: sawTerm, int: sawInt },
				killAction,
			};
		},
		"command-bar-interactions": async (receipt, check, captureSample) => {
			await runFixture("stty-size.sh", ["before-actions"]);
			const beforeText = await waitForText("NN30-STTY-before-actions", 8_000);
			const beforeSize = beforeText.match(
				/NN30-STTY-before-actions rows=(\d+) cols=(\d+)/,
			);
			await openTerminalActions();
			const [dialog, elements] = await Promise.all([
				dialogState(),
				driver.getElements(
					{ target: ACTIONS_TARGET, limit: 400 },
					{ timeoutMs: 8_000 },
				) as Promise<Json>,
			]);
			const actionIds = [
				...new Set(
					walk(elements)
						.flatMap((node) => {
							const direct = node.actionId ?? node.action_id;
							const semantic = String(node.semanticId ?? node.id ?? "");
							const suffix = /(?:action|row)/.test(semantic)
								? semantic.split(":").at(-1)
								: undefined;
							return [direct, suffix];
						})
						.filter(
							(value): value is string =>
								typeof value === "string" && value.length > 0,
						),
				),
			].sort((a, b) => a.localeCompare(b));
			const required = [
				"clear",
				"copy_all",
				"find",
				"reset",
				"scroll_to_bottom",
			];
			const missing = required.filter((id) => !actionIds.includes(id));
			check(
				"terminal_command_bar_exposes_required_stable_actions",
				missing.length === 0,
				"product",
				{
					actionIds,
					required,
					missing,
					dialog,
				},
			);
			await gpuiKey("k", ["cmd"], MAIN_TARGET);
			await poll(
				"terminal actions toggle closes",
				actionsWindows,
				(value) => value.length === 0,
				8_000,
			);

			const toggleSamples: Json[] = [];
			for (let cycle = 0; cycle < 4; cycle += 1) {
				await gpuiKey("k", ["cmd"], MAIN_TARGET);
				const opened = await poll(
					`actions open cycle ${cycle}`,
					actionsWindows,
					(value) => value.length === 1,
					8_000,
				);
				toggleSamples.push({
					cycle,
					phase: "open",
					count: opened.length,
					capturedAt: new Date().toISOString(),
				});
				await gpuiKey("k", ["cmd"], MAIN_TARGET);
				const closed = await poll(
					`actions close cycle ${cycle}`,
					actionsWindows,
					(value) => value.length === 0,
					8_000,
				);
				toggleSamples.push({
					cycle,
					phase: "close",
					count: closed.length,
					capturedAt: new Date().toISOString(),
				});
			}
			check(
				"rapid_command_bar_toggle_never_duplicates_popup",
				toggleSamples.every(
					(sample) => sample.count === (sample.phase === "open" ? 1 : 0),
				),
				"product",
				{ toggleSamples },
			);

			let findAction: Json | null = null;
			let findSurface: Json | undefined;
			if (actionIds.includes("find")) {
				findAction = await activateTerminalAction("find", "Find");
				await Bun.sleep(100);
				const afterFindElements = await terminalElements();
				findSurface = walk(afterFindElements).find((node) => {
					const semantic = String(
						node.semanticId ?? node.id ?? "",
					).toLowerCase();
					const role = String(node.role ?? "").toLowerCase();
					return (
						semantic.includes("terminal-search") ||
						semantic.includes("terminal-find") ||
						role === "searchbox"
					);
				});
			}
			check(
				"find_action_opens_observable_terminal_search",
				Boolean(findSurface),
				"product",
				{
					findActionPresent: actionIds.includes("find"),
					findSurface: findSurface ?? null,
					contract:
						"Displayed Find action says Search in terminal output; it must not silently no-op.",
				},
			);
			if (findSurface) {
				await gpuiKey("escape", [], MAIN_TARGET);
				await poll(
					"terminal search dismiss",
					state,
					(value) => value.promptType === VIEW,
					8_000,
				);
			}

			const clearAction = actionIds.includes("clear")
				? await activateTerminalAction("clear", "Clear Terminal")
				: null;
			await runFixture("stty-size.sh", ["after-actions"]);
			await typeRaw("printf 'NN30-COMMAND-BAR-RESTORED\\n'\r");
			const afterText = await waitForText("NN30-COMMAND-BAR-RESTORED", 8_000);
			const afterSize = afterText.match(
				/NN30-STTY-after-actions rows=(\d+) cols=(\d+)/,
			);
			const sameSize =
				beforeSize?.[1] === afterSize?.[1] &&
				beforeSize?.[2] === afterSize?.[2];
			check(
				"command_bar_restores_terminal_focus_and_grid_size",
				Boolean(afterSize) && sameSize,
				"product",
				{
					before: beforeSize
						? { rows: beforeSize[1], cols: beforeSize[2] }
						: null,
					after: afterSize ? { rows: afterSize[1], cols: afterSize[2] } : null,
					focusMarker: afterText.includes("NN30-COMMAND-BAR-RESTORED"),
				},
			);
			await captureSample("after-command-bar-interactions");
			receipt.evidence = {
				actionIds,
				dialog,
				toggleSamples,
				findAction,
				findSurface: findSurface ?? null,
				clearAction,
				beforeSize: beforeSize?.slice(1) ?? null,
				afterSize: afterSize?.slice(1) ?? null,
			};
		},
		"theme-hot-reload": async (receipt, check, captureSample) => {
			const boundsDigest = (layout: Json) =>
				JSON.stringify(
					((layout.components ?? []) as Json[])
						.map((component) => ({
							name: component.name ?? component.type ?? component.id ?? null,
							bounds: component.bounds ?? component.visibleBounds ?? null,
						}))
						.sort((a, b) => String(a.name).localeCompare(String(b.name))),
				);
			const beforeLayout = (await driver.getLayoutInfo(
				{ target: MAIN_TARGET },
				{ timeoutMs: 8_000 },
			)) as Json;
			const beforeLog = readFileSync(driver.logPath, "utf8");
			const beforePropagationCount =
				beforeLog.split("Theme propagated to terminal").length - 1;
			const templatePath = join(
				PROJECT_ROOT,
				"tests/theme/snapshots/theme_dark_default.json",
			);
			let template: Json;
			try {
				template = JSON.parse(readFileSync(templatePath, "utf8")) as Json;
			} catch (error) {
				throw new HarnessInvalid(
					`theme fixture parse failed: ${error instanceof Error ? error.message : String(error)}`,
				);
			}
			const colors = template.colors as Json;
			(colors.background as Json).main = "#101820";
			(colors.terminal as Json).red = "#12AB34";
			const themePath = join(
				driver.sessionDir,
				"home",
				".scriptkit",
				"theme.json",
			);
			writeFileSync(
				themePath,
				`${JSON.stringify(template, null, 2)}\n`,
				"utf8",
			);
			let propagatedLog = beforeLog;
			const propagationDeadline = performance.now() + 5_000;
			while (performance.now() < propagationDeadline) {
				propagatedLog = readFileSync(driver.logPath, "utf8");
				if (
					propagatedLog.split("Theme propagated to terminal").length - 1 >
					beforePropagationCount
				)
					break;
				await Bun.sleep(50);
			}
			await typeRaw("printf '\\033[31mNN30-THEME-ANSI\\033[0m\\n'\r");
			const terminalAfter = await waitForText("NN30-THEME-ANSI", 8_000);
			const afterLayout = (await driver.getLayoutInfo(
				{ target: MAIN_TARGET },
				{ timeoutMs: 8_000 },
			)) as Json;
			check(
				"theme_watcher_propagates_to_open_terminal",
				propagatedLog.includes("Theme propagated to terminal"),
				"product",
				{
					beforePropagationCount,
					afterPropagationCount:
						propagatedLog.split("Theme propagated to terminal").length - 1,
					themePath,
				},
			);
			check(
				"terminal_renders_after_theme_hot_reload",
				terminalAfter.includes("NN30-THEME-ANSI"),
				"product",
				{
					marker: "NN30-THEME-ANSI",
				},
			);
			check(
				"theme_hot_reload_preserves_terminal_geometry",
				boundsDigest(beforeLayout) === boundsDigest(afterLayout),
				"product",
				{
					beforeBoundsDigest: boundsDigest(beforeLayout),
					afterBoundsDigest: boundsDigest(afterLayout),
				},
			);
			await captureSample("after-theme-hot-reload");
			receipt.evidence = {
				templatePath,
				themePath,
				changedTokens: {
					"colors.background.main": "#101820",
					"colors.terminal.red": "#12AB34",
				},
				beforePropagationCount,
				afterPropagationCount:
					propagatedLog.split("Theme propagated to terminal").length - 1,
			};
		},
	};

	try {
		for (const rowId of rows) await executeRow(rowId, rowBodies[rowId]);
		try {
			driver.send({ type: "triggerBuiltin", name: "mainList" });
			await poll(
				"terminal cleanup to main list",
				state,
				(value) => value.promptType !== VIEW,
				8_000,
			);
			driver.send({ type: "hide" });
			const hidden = await poll(
				"main window hidden",
				state,
				(value) => value.windowVisible === false,
				8_000,
			);
			summary.cleanup = {
				windowVisible: hidden.windowVisible ?? null,
				windows: await windows(),
			};
		} catch (error) {
			summary.harnessFindings.push({
				rowId: "cleanup",
				name: "cleanup_unobservable",
				detail: {
					error: error instanceof Error ? error.message : String(error),
				},
			});
		}
	} finally {
		summary.sessionDir = driver.sessionDir;
		summary.appLog = driver.logPath;
		await driver.close();
		const escapedBinary = BINARY.replace(/[\^$.*+?()[\]{}|]/g, "\\$&");
		const escapedFixtures = fixtures.dir.replace(/[\^$.*+?()[\]{}|]/g, "\\$&");
		const processPatterns = [
			`^${escapedBinary}([[:space:]]|$)`,
			escapedFixtures,
		];
		const probes = processPatterns.map((pattern) => {
			const probe = Bun.spawnSync(["pgrep", "-f", pattern], {
				stdout: "pipe",
				stderr: "pipe",
			});
			const stdout = new TextDecoder().decode(probe.stdout).trim();
			return {
				command: ["pgrep", "-f", pattern],
				exitCode: probe.exitCode,
				stdout,
				stderr: new TextDecoder().decode(probe.stderr).trim(),
				clean: probe.exitCode === 1 && stdout.length === 0,
			};
		});
		const postTeardown = {
			checkedAt: new Date().toISOString(),
			probes,
			clean: probes.every((probe) => probe.clean),
		};
		summary.postTeardownProcessCheck = postTeardown;
		await writeJson(
			join(OUTPUT_DIR, "post-teardown-process.json"),
			postTeardown,
		);
		if (!postTeardown.clean)
			summary.harnessFindings.push({
				rowId: "postTeardown",
				name: "terminal_or_fixture_process_remained",
				detail: postTeardown,
			});
	}

	summary.classification = summary.environmentFindings.length
		? "blocked-by-environment"
		: summary.harnessFindings.length
			? "invalid-harness"
			: summary.productFindings.length
				? "failed-product"
				: "verified";
	summary.pass = summary.classification === "verified";
	await writeJson(join(OUTPUT_DIR, "summary.json"), summary);
	console.log(JSON.stringify(summary, null, 2));
	process.exitCode = summary.pass
		? 0
		: summary.classification === "blocked-by-environment"
			? 3
			: summary.classification === "invalid-harness"
				? 2
				: 1;
}

await runBattery();

#!/usr/bin/env bun
/**
 * NN=29 AI feature shell chaos probe.
 *
 * Safety contract:
 * - Every row launches a fresh scratch HOME (`sandboxHome: true`).
 * - Profile-shell rows never seed auth, submit an AI prompt, or execute a script.
 * - Fixtures are written only below the driver's scratch HOME.
 *
 * Run one row at a time:
 *   bun scripts/agentic/chaos-ai-feature-shell-probe.ts a1-valid-current
 */
import {
	mkdirSync,
	readFileSync,
	readdirSync,
	realpathSync,
	unlinkSync,
	writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Driver } from "../devtools/driver.ts";

const LANE = process.env.NN29_LANE ?? "finder-ai-2";
const BINARY =
	process.env.SCRIPT_KIT_GPUI_BINARY ??
	`target-agent/artifacts/${LANE}/script-kit-gpui`;
const ROW = process.argv[2] ?? "a1-valid-current";
const PROBE_DIR = dirname(fileURLToPath(import.meta.url));
const FLOW_FIXTURE = resolve(PROBE_DIR, "fixtures/flow-ux-project");
const FLOW_PACKAGE_FIXTURE = resolve(PROBE_DIR, "fixtures/flow-desk-package");
const IMPLEMENTED_ROWS = new Set([
	"a1-valid-current",
	"a2-malformed",
	"a3-outdated-remnants",
	"a4-hostile-valid",
	"a5-create",
	"a6-deleted-selected-heals",
	"c3-app-positional-turn",
]);

if (!IMPLEMENTED_ROWS.has(ROW)) {
	console.error(`Unknown or not-yet-implemented row: ${ROW}`);
	process.exit(2);
}

const receipt: Record<string, unknown> = {
	probe: "chaos-ai-feature-shell",
	battery: "NN=29",
	row: ROW,
	binary: BINARY,
	safety: {
		sandboxHome: true,
		seedAgentAuth: false,
		aiPromptSubmitted: false,
		scriptExecuted: false,
		showSent: false,
	},
};

let driver: Driver | undefined;
let exitCode = 1;
console.error(`[driver] binary: ${BINARY} (pinned NN=29 ${LANE} artifact)`);

function findSemanticElement(
	value: unknown,
	semanticId: string,
): Record<string, unknown> | null {
	if (Array.isArray(value)) {
		for (const item of value) {
			const found = findSemanticElement(item, semanticId);
			if (found) return found;
		}
		return null;
	}
	if (value && typeof value === "object") {
		const object = value as Record<string, unknown>;
		if (object.semanticId === semanticId || object.semantic_id === semanticId)
			return object;
		for (const child of Object.values(object)) {
			const found = findSemanticElement(child, semanticId);
			if (found) return found;
		}
	}
	return null;
}

async function waitForCurrentProfile(
	activeDriver: Driver,
	profileId: string,
	timeoutMs: number,
): Promise<{
	elements: unknown;
	current: Record<string, unknown> | null;
	waitedMs: number;
}> {
	const startedAt = performance.now();
	let elements: unknown = {};
	let current: Record<string, unknown> | null = null;
	while (performance.now() - startedAt < timeoutMs) {
		elements = await activeDriver.getElements(
			{ limit: 300 },
			{ timeoutMs: 5_000 },
		);
		current = findSemanticElement(elements, "status:profile-search-current");
		if (current?.value === profileId) {
			return {
				elements,
				current,
				waitedMs: Math.round(performance.now() - startedAt),
			};
		}
		await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
	}
	return {
		elements,
		current,
		waitedMs: Math.round(performance.now() - startedAt),
	};
}

async function waitForLogNeedle(
	activeDriver: Driver,
	needle: string,
	timeoutMs: number,
): Promise<boolean> {
	const startedAt = performance.now();
	while (performance.now() - startedAt < timeoutMs) {
		const logs = await activeDriver.getLogs({ limit: 500 });
		if (JSON.stringify(logs).includes(needle)) return true;
		await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
	}
	return false;
}

async function waitForElementNeedle(
	activeDriver: Driver,
	needle: string,
	timeoutMs: number,
): Promise<{ elements: unknown; waitedMs: number }> {
	const startedAt = performance.now();
	let elements: unknown = {};
	while (performance.now() - startedAt < timeoutMs) {
		elements = await activeDriver.getElements(
			{ limit: 300 },
			{ timeoutMs: 5_000 },
		);
		if (JSON.stringify(elements).includes(needle)) {
			return { elements, waitedMs: Math.round(performance.now() - startedAt) };
		}
		await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
	}
	return { elements, waitedMs: Math.round(performance.now() - startedAt) };
}

try {
	driver = await Driver.launch({
		sessionName: `${LANE}-${ROW}`,
		binary: BINARY,
		sandboxHome: true,
		seedAgentAuth: false,
		env:
			ROW === "c3-app-positional-turn"
				? {
						SCRIPT_KIT_FLOW_UX_CWD: FLOW_FIXTURE,
						SCRIPT_KIT_FLOWS_PACKAGE_DIR: FLOW_PACKAGE_FIXTURE,
						SCRIPT_KIT_FLOWS_BIN_DIR: resolve(FLOW_PACKAGE_FIXTURE, "bin"),
						SCRIPT_KIT_CODEX_BIN: resolve(
							FLOW_PACKAGE_FIXTURE,
							"bin/fake-codex",
						),
						PATH: `${resolve(FLOW_FIXTURE, "bin")}:${resolve(
							FLOW_PACKAGE_FIXTURE,
							"bin",
						)}:${process.env.PATH ?? ""}`,
					}
				: { EDITOR: "/bin/echo" },
	});
	await driver.waitForSettle();

	if (ROW === "c3-app-positional-turn") {
		let sawVisible = false;
		let lastStateSample = "";
		const stateSamples: Record<string, unknown>[] = [];
		const flowUx = (state: Record<string, unknown>) =>
			(state.flowUx as Record<string, unknown> | undefined) ?? {};
		const lastSession = (state: Record<string, unknown>) => {
			const sessions = flowUx(state).sessions;
			return Array.isArray(sessions)
				? (sessions.at(-1) as Record<string, unknown> | undefined)
				: undefined;
		};
		const pollFlowState = async (
			predicate: (state: Record<string, unknown>) => boolean,
			timeoutMs: number,
		) => {
			const startedAt = performance.now();
			let state: Record<string, unknown> = {};
			while (performance.now() - startedAt < timeoutMs) {
				state = (await driver?.getState({ timeoutMs: 5_000 })) as Record<
					string,
					unknown
				>;
				sawVisible ||= state.windowVisible === true;
				const session = lastSession(state);
				const sample = {
					promptType: state.promptType,
					windowVisible: state.windowVisible,
					selectedFlowId: flowUx(state).selectedFlowId,
					session: session
						? {
								flowId: session.flowId,
								state: session.state,
								turns: session.turns,
								turnInFlight: session.turnInFlight,
								transport: session.transport,
							}
						: undefined,
				};
				const sampleSignature = JSON.stringify(sample);
				if (sampleSignature !== lastStateSample) {
					stateSamples.push(sample);
					lastStateSample = sampleSignature;
				}
				if (predicate(state)) return state;
				await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
			}
			return state;
		};

		driver.send({ type: "triggerBuiltin", builtinId: "builtin/flows" });
		const deskState = await pollFlowState(
			(state) =>
				(flowUx(state).roster as { status?: unknown } | undefined)?.status ===
				"ready",
			10_000,
		);
		await driver.setFilterAndWait("nn29-positional-turn", {
			timeoutMs: 10_000,
		});
		await pollFlowState(
			(state) =>
				flowUx(state).selectedFlowId === "project:nn29-positional-turn.fasteng",
			10_000,
		);
		driver.simulateKey("enter");
		const openedState = await pollFlowState(
			(state) =>
				state.promptType === "flowSession" &&
				lastSession(state)?.flowId === "project:nn29-positional-turn.fasteng",
			10_000,
		);
		await driver.request(
			{
				type: "batch",
				commands: [{ type: "setInput", text: "NN29 app runtime turn" }],
				options: {
					stopOnError: true,
					rollbackOnError: false,
					timeout: 5_000,
				},
			},
			{ expect: "batchResult", timeoutMs: 8_000 },
		);
		await driver.simulateGpuiEvent(
			{ type: "keyDown", key: "enter", modifiers: [] },
			{ target: { type: "main" }, timeoutMs: 5_000 },
		);
		const completedState = await pollFlowState((state) => {
			const session = lastSession(state);
			return session?.turns === 1 || session?.state === "error";
		}, 20_000);
		const session = lastSession(completedState);
		const elements = await driver.getElements(
			{ limit: 400 },
			{ timeoutMs: 8_000 },
		);
		const logs = await driver.getLogs({ limit: 500 });
		const elementsBlob = JSON.stringify(elements);
		const logsBlob = JSON.stringify(logs);
		const checks: Record<string, boolean> = {
			deskRosterReady:
				(flowUx(deskState).roster as { status?: unknown } | undefined)
					?.status === "ready",
			positionalFlowSelected:
				lastSession(openedState)?.flowId ===
				"project:nn29-positional-turn.fasteng",
			flowSessionOpened: openedState.promptType === "flowSession",
			mdflowTransport: session?.transport === "mdflowTurns",
			turnCompleted: session?.turns === 1 && session?.state === "needs you",
			engineReplyVisible: elementsBlob.includes("FASTENG_OK"),
			taskReachedEngine: elementsBlob.includes("NN29 app runtime turn"),
			noLegacyTaskShapeFailure:
				!logsBlob.includes("UNUSED_VARIABLE_FLAG") &&
				!logsBlob.includes("Missing template variables: _1"),
			hiddenThroughout: !sawVisible,
			appAlive: driver.alive,
		};
		receipt.sessionDir = driver.sessionDir;
		receipt.scratchHome = resolve(driver.sessionDir, "home");
		receipt.observed = {
			checks,
			session,
			stateSamples,
			legacyFailureInLogs:
				logsBlob.includes("UNUSED_VARIABLE_FLAG") ||
				logsBlob.includes("Missing template variables: _1"),
		};
		receipt.pass = Object.values(checks).every(Boolean);
		receipt.failedChecks = Object.entries(checks)
			.filter(([, passed]) => !passed)
			.map(([name]) => name);
		exitCode = receipt.pass ? 0 : 1;
	} else {
		const scratchHome = resolve(driver.sessionDir, "home");
		const profilesDir = resolve(scratchHome, ".scriptkit", "profiles");
		mkdirSync(profilesDir, { recursive: true });
		const seededPaths: string[] = [];
		const seedProfile = (filename: string, lines: string[]): void => {
			const path = resolve(profilesDir, filename);
			mkdirSync(dirname(path), { recursive: true });
			writeFileSync(path, lines.join("\n"));
			const realPath = realpathSync(path);
			if (!realPath.startsWith(`${realpathSync(scratchHome)}/`)) {
				throw new Error(`fixture escaped scratch HOME: ${realPath}`);
			}
			seededPaths.push(realPath);
		};

		seedProfile("nn29-valid.md", [
			"---",
			"name: NN29 Valid Current",
			"provider: openai-codex",
			"model: gpt-5.4",
			"tools:",
			"  - web_search",
			"no-session: true",
			"---",
			"",
			"This is a deterministic profile-shell fixture. Do not run anything.",
			"",
		]);

		if (ROW === "a2-malformed") {
			seedProfile("nn29-bad-frontmatter.md", [
				"---",
				"name: [unterminated",
				"model: openai-codex/gpt-5.4",
				"---",
				"This malformed profile must not poison its valid sibling.",
			]);
			seedProfile("nn29-wrong-types.md", [
				"---",
				"name:",
				"  - not",
				"  - a scalar",
				"model:",
				"  nested: wrong",
				"tools:",
				"  - nested: wrong",
				"no-session: definitely-not-a-boolean",
				"---",
				"This wrong-typed profile must not silently become a different profile.",
			]);
		} else if (ROW === "a3-outdated-remnants") {
			seedProfile("profile.json", [
				'{"id":"plugin:legacy/direct","name":"NN29 Legacy Direct JSON","model":"stale-model"}',
			]);
			const legacyPath = resolve(
				scratchHome,
				".scriptkit",
				"plugins",
				"nn29-legacy-plugin",
				"profiles",
				"stale",
				"profile.json",
			);
			mkdirSync(dirname(legacyPath), { recursive: true });
			writeFileSync(
				legacyPath,
				'{"id":"plugin:nn29-legacy/stale","name":"NN29 Legacy Nested JSON","model":"stale-model"}',
			);
			const legacyRealPath = realpathSync(legacyPath);
			if (!legacyRealPath.startsWith(`${realpathSync(scratchHome)}/`)) {
				throw new Error(`fixture escaped scratch HOME: ${legacyRealPath}`);
			}
			seededPaths.push(legacyRealPath);
		} else if (ROW === "a4-hostile-valid") {
			seedProfile("nn29-huge.md", [
				"---",
				"name: NN29 Huge 1MiB",
				"model: openai-codex/gpt-5.4",
				"no-session: true",
				"---",
				"",
				"H".repeat(1024 * 1024),
			]);
			seedProfile("nn29-zalgo.md", [
				"---",
				"name: NN29 Zalgo Z̴̙̓͗a̷̻͒l̵͎͋g̶̯͗o̴̰̕ 👩🏽‍💻 مرحبا",
				"model: openai-codex/gpt-5.4",
				"no-session: true",
				"---",
				"",
				"Combining marks, emoji ZWJ, and RTL must remain inert profile text.",
			]);
		}

		driver.simulateKey("tab", ["shift"]);
		await driver.waitForSettle({ timeoutMs: 8_000 });

		const profileRowNeedle = "profile-search-row:nn29-valid";
		const [state, elementWait] = await Promise.all([
			driver.getState({ timeoutMs: 5_000 }),
			waitForElementNeedle(driver, profileRowNeedle, 5_000),
		]);
		const hostileEvidence: Record<string, unknown> = {};
		const createEvidence: Record<string, unknown> = {};
		const deletionEvidence: Record<string, unknown> = {};
		if (ROW === "a4-hostile-valid") {
			await driver.setFilterAndWait("nn29 huge", { timeoutMs: 8_000 });
			const hugeWait = await waitForElementNeedle(
				driver,
				"profile-search-row:nn29-huge",
				8_000,
			);
			hostileEvidence.huge = {
				visible: JSON.stringify(hugeWait.elements).includes(
					"profile-search-row:nn29-huge",
				),
				waitedMs: hugeWait.waitedMs,
			};
			await driver.setFilterAndWait("nn29 zalgo", { timeoutMs: 8_000 });
			const zalgoWait = await waitForElementNeedle(
				driver,
				"profile-search-row:nn29-zalgo",
				8_000,
			);
			hostileEvidence.zalgo = {
				visible: JSON.stringify(zalgoWait.elements).includes(
					"profile-search-row:nn29-zalgo",
				),
				waitedMs: zalgoWait.waitedMs,
			};
		} else if (ROW === "a5-create") {
			await driver.setFilterAndWait("create", { timeoutMs: 8_000 });
			const createWait = await waitForElementNeedle(
				driver,
				"profile-search-row:create-new-profile",
				8_000,
			);
			driver.simulateKey("enter");
			const createLogged = await waitForLogNeedle(
				driver,
				"profile_search_create_profile",
				8_000,
			);
			await driver.waitForSettle({ timeoutMs: 8_000 });
			const createdName = readdirSync(profilesDir).find(
				(name) => name.endsWith(".md") && name !== "nn29-valid.md",
			);
			const createdPath = createdName
				? resolve(profilesDir, createdName)
				: null;
			const createdBody = createdPath ? readFileSync(createdPath, "utf8") : "";
			createEvidence.rowVisible = JSON.stringify(createWait.elements).includes(
				"profile-search-row:create-new-profile",
			);
			createEvidence.createLogged = createLogged;
			createEvidence.createdPath = createdPath;
			createEvidence.templateValid =
				createdBody.includes("name: My Profile") &&
				createdBody.includes("no-session: true") &&
				createdBody.includes("You are a focused Agent Chat profile");
			createEvidence.returnedToScriptList = JSON.stringify(
				await driver.getState({ timeoutMs: 5_000 }),
			).includes('"promptType":"none"');
		} else if (ROW === "a6-deleted-selected-heals") {
			await driver.setFilterAndWait("NN29 Valid Current", { timeoutMs: 8_000 });
			const selectedWait = await waitForElementNeedle(
				driver,
				profileRowNeedle,
				8_000,
			);
			driver.simulateKey("enter");
			const persisted = await waitForLogNeedle(
				driver,
				"profile_search_profile_persisted",
				8_000,
			);
			await driver.waitForSettle({ timeoutMs: 8_000 });
			unlinkSync(resolve(profilesDir, "nn29-valid.md"));
			driver.simulateKey("tab", ["shift"]);
			await driver.waitForSettle({ timeoutMs: 8_000 });
			const healed = await waitForCurrentProfile(driver, "brain", 8_000);
			deletionEvidence.selectedRowWasVisible = JSON.stringify(
				selectedWait.elements,
			).includes(profileRowNeedle);
			deletionEvidence.persisted = persisted;
			deletionEvidence.current = healed.current;
			deletionEvidence.healWaitMs = healed.waitedMs;
			deletionEvidence.deletedRowAbsent = !JSON.stringify(
				healed.elements,
			).includes(profileRowNeedle);
		}
		const logs = await driver.getLogs({ limit: 500 });
		const elementsBlob = JSON.stringify(elementWait.elements);
		const logsBlob = JSON.stringify(logs);
		const checks: Record<string, boolean> = {
			profileSearchOpen: JSON.stringify(state).includes("ProfileSearch"),
			validRowVisible: elementsBlob.includes(profileRowNeedle),
			createRowVisible: elementsBlob.includes(
				"profile-search-row:create-new-profile",
			),
			appAlive: driver.alive,
		};
		if (ROW === "a1-valid-current") {
			checks.noParseFailure = !logsBlob.includes("mdflow_profile_parse_failed");
		} else if (ROW === "a2-malformed") {
			checks.badFrontmatterOmitted = !elementsBlob.includes(
				"profile-search-row:nn29-bad-frontmatter",
			);
			checks.badFrontmatterWarned =
				logsBlob.includes("mdflow_profile_parse_failed") &&
				logsBlob.includes("nn29-bad-frontmatter.md");
			checks.wrongTypesOmitted = !elementsBlob.includes(
				"profile-search-row:nn29-wrong-types",
			);
			checks.wrongTypesWarned = logsBlob.includes("nn29-wrong-types.md");
		} else if (ROW === "a3-outdated-remnants") {
			checks.directJsonIgnored =
				!elementsBlob.includes("plugin:legacy/direct") &&
				!elementsBlob.includes("NN29 Legacy Direct JSON");
			checks.nestedPluginJsonIgnored =
				!elementsBlob.includes("plugin:nn29-legacy/stale") &&
				!elementsBlob.includes("NN29 Legacy Nested JSON");
			checks.noOutdatedParseNoise =
				!logsBlob.includes("NN29 Legacy Direct JSON") &&
				!logsBlob.includes("NN29 Legacy Nested JSON");
		} else if (ROW === "a4-hostile-valid") {
			checks.hugeProfileVisibleAndFilterable =
				(hostileEvidence.huge as { visible?: boolean }).visible === true;
			checks.zalgoProfileVisibleAndFilterable =
				(hostileEvidence.zalgo as { visible?: boolean }).visible === true;
			checks.noHostileParseFailure = !logsBlob.includes(
				"mdflow_profile_parse_failed",
			);
		} else if (ROW === "a5-create") {
			checks.createRowSelectable = createEvidence.rowVisible === true;
			checks.createEventLogged = createEvidence.createLogged === true;
			checks.templateCreatedInScratch = createEvidence.templateValid === true;
			checks.returnedToScriptList =
				createEvidence.returnedToScriptList === true;
			checks.inertEditorOnly = logsBlob.includes("Opening file in editor");
		} else {
			checks.selectedProfilePersisted = deletionEvidence.persisted === true;
			checks.deletedProfileRowHealed =
				deletionEvidence.deletedRowAbsent === true;
			checks.currentProfileFellBackToBrain =
				(deletionEvidence.current as { value?: unknown } | undefined)?.value ===
				"brain";
		}

		receipt.sessionDir = driver.sessionDir;
		receipt.scratchHome = scratchHome;
		receipt.seededProfiles = seededPaths;
		receipt.observed = {
			state,
			profileRowNeedle,
			cacheRefreshWaitMs: elementWait.waitedMs,
			hostileEvidence,
			createEvidence,
			deletionEvidence,
			checks,
		};
		receipt.pass = Object.values(checks).every(Boolean);
		receipt.failedChecks = Object.entries(checks)
			.filter(([, passed]) => !passed)
			.map(([name]) => name);
		exitCode = receipt.pass ? 0 : 1;
	}
} catch (error) {
	const message = error instanceof Error ? error.message : String(error);
	const environmentBlocked =
		message.includes("did not become ready") ||
		message.includes("Pi Agent Chat is unavailable") ||
		message.includes("Binary not found");
	receipt.pass = false;
	receipt.classification = environmentBlocked
		? "ENV"
		: "HARNESS_OR_PRODUCT_RED";
	receipt.error = message;
} finally {
	if (driver) {
		driver.send({ type: "hide" });
		const hidden = await driver
			.waitForState({ windowVisible: false }, { timeoutMs: 4_000 })
			.then(() => true)
			.catch(() => false);
		receipt.cleanup = { hidden, pid: driver.pid ?? null };
		await driver.close();
		receipt.finalization = driver.finalization;
	}
}

console.log(JSON.stringify(receipt, null, 2));
process.exit(exitCode);

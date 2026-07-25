#!/usr/bin/env bun
// Does a flow session preserve a multi-line message?
//
// Audit finding (docs/specs/one-conversation-experience.md §2.1): it did not.
// Flow composes in the shared main-window input, which is single-line, and the
// vendored gpui-component `paste()` used to run
//
//     if !self.mode.is_multi_line() { new_text = new_text.replace('\n', "") }
//
// Newlines were DELETED rather than flattened, so
//   "Fix the bug\nin auth.rs"  ->  "Fix the bugin auth.rs"
// inventing the word "bugin". Nothing warned, because the app-side guard that
// WOULD log (`filter_change.newline_ignored`) never saw a newline — the vendor
// had already removed it.
//
// This probe recorded that corruption at runtime, then verified the fix: each
// run of line breaks now collapses to a single space, so every word and word
// boundary survives.
//
// It stays falsifiable in every direction: it records the exact string the
// composer holds and classifies it as intact / flattened / corrupted /
// unexpected, rather than asserting an outcome it cannot observe. Line
// structure is still lost — that needs a dedicated Flow composer and is
// tracked under "unify the AI composer", so a flattened run reports the open
// gap in `structureLossStillOpen` instead of quietly reading as complete.
//
// Run:
//   SCRIPT_KIT_AGENT_ARTIFACT_NAME=ai-rock-solid \
//     ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui
//   SCRIPT_KIT_GPUI_BINARY="$PWD/target-agent/artifacts/ai-rock-solid/script-kit-gpui" \
//     bun scripts/agentic/flow-composer-multiline-probe.ts
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver.ts";

const repoRoot = resolve(import.meta.dir, "../..");
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const receiptPath = resolve(
	process.env.PROBE_RECEIPT ??
		".test-output/ai-rock-solid-ux/flow-composer-multiline.json",
);
const binary = process.env.SCRIPT_KIT_GPUI_BINARY;

/** The message a user would realistically paste into a chat. */
const MULTILINE = "Fix the bug\nin auth.rs";
/** What a newline-DELETING single-line input turns that into. */
const CORRUPTED_IF_DELETED = "Fix the bugin auth.rs";
/**
 * What a newline-FLATTENING single-line input turns that into.
 *
 * This is the accepted outcome while Flow still composes in the shared
 * single-line main input: line structure is lost, but no word is invented and
 * no character is dropped. Real newline retention needs a dedicated Flow
 * composer, tracked separately as "unify the AI composer".
 */
const FLATTENED_TO_SPACES = "Fix the bug in auth.rs";

const failures: string[] = [];
const observed: Record<string, unknown> = {};

await mkdir(resolve(".test-output/ai-rock-solid-ux"), { recursive: true });

/** Save the user's pasteboard so a probe never eats their clipboard. */
async function readPasteboard(): Promise<string> {
	const p = Bun.spawn(["pbpaste"], { stdout: "pipe" });
	return await new Response(p.stdout).text();
}
async function writePasteboard(text: string): Promise<void> {
	const p = Bun.spawn(["pbcopy"], { stdin: "pipe" });
	p.stdin.write(text);
	p.stdin.end();
	await p.exited;
}

async function until<T>(
	probe: () => Promise<T | null | undefined>,
	timeoutMs: number,
): Promise<T | null> {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		const value = await probe();
		if (value) return value;
		await sleep(200);
	}
	return null;
}

async function pressKey(d: Driver, key: string, modifiers?: string[]) {
	await d.simulateGpuiEvent({
		type: "keyDown",
		key,
		...(modifiers ? { modifiers } : {}),
	});
	await sleep(150);
}

const savedClipboard = await readPasteboard();
let d: Driver | undefined;

try {
	d = await Driver.launch({
		sandboxHome: true,
		sessionName: "flow-multiline",
		...(binary ? { binary } : {}),
		env: { SCRIPT_KIT_FLOW_UX_CWD: repoRoot },
	});
	await d.request({ type: "show" });
	await d.waitForSettle();

	// Open a flow session. Poll BOTH the selection and the resulting
	// promptType: a fixed sleep after Enter fires against whatever row happens
	// to be selected on a slow launch (learned the hard way in S12).
	const opened = await until(async () => {
		await d!.setFilterAndWait("scout");
		const selected = await until(async () => {
			const st: any = await d!.getState();
			return typeof st?.selectedValue === "string" &&
				st.selectedValue.toLowerCase().includes("scout")
				? st
				: null;
		}, 12_000);
		if (!selected) return null;
		await pressKey(d!, "enter");
		return await until(async () => {
			const st: any = await d!.getState();
			return st?.promptType === "flowSession" ? st : null;
		}, 10_000);
	}, 45_000);

	if (!opened) {
		failures.push("flow session never opened — nothing was measured");
	} else {
		// Clear whatever the composer holds, then paste the multi-line message.
		await d.setFilterAndWait("");
		await writePasteboard(MULTILINE);
		await sleep(200);
		await pressKey(d, "v", ["cmd"]);
		await sleep(500);

		const after: any = await d.getState();
		const composer: string = after?.inputValue ?? "";

		observed.promptType = after?.promptType ?? null;
		observed.pasted = MULTILINE;
		observed.composerAfterPaste = composer;
		observed.composerHasNewline = composer.includes("\n");
		observed.matchesDeletedNewline = composer === CORRUPTED_IF_DELETED;
		observed.matchesPastedExactly = composer === MULTILINE;
		observed.matchesFlattenedToSpaces = composer === FLATTENED_TO_SPACES;

		if (composer === "") {
			failures.push(
				"composer was empty after ⌘V — the paste never reached the input, " +
					"so this run proves nothing about newline handling",
			);
		} else if (composer === MULTILINE) {
			// The fix is in: newlines survived.
			observed.verdict = "intact";
		} else if (composer === FLATTENED_TO_SPACES) {
			// Words and word boundaries survived. Line structure did not, which
			// is the known limit of composing in the shared single-line input,
			// tracked separately under "unify the AI composer".
			observed.verdict = "flattened-words-preserved";
			observed.structureLossStillOpen = true;
		} else if (composer === CORRUPTED_IF_DELETED) {
			observed.verdict = "corrupted-newline-deleted";
			failures.push(
				`DATA LOSS confirmed at runtime: pasted ${JSON.stringify(MULTILINE)} ` +
					`but the composer holds ${JSON.stringify(composer)} — the newline was ` +
					"deleted, welding two words together",
			);
		} else {
			observed.verdict = "unexpected";
			failures.push(
				`composer holds ${JSON.stringify(composer)}, which is neither the ` +
					"pasted text, the flattened form, nor the known newline-deleted " +
					"form — investigate before drawing a conclusion",
			);
		}

		// Shift+Enter is the other half of the same defect.
		await d.setFilterAndWait("line one");
		await pressKey(d, "enter", ["shift"]);
		await sleep(300);
		const afterShiftEnter: any = await d.getState();
		observed.composerAfterShiftEnter = afterShiftEnter?.inputValue ?? "";
		observed.shiftEnterInsertedNewline = String(
			afterShiftEnter?.inputValue ?? "",
		).includes("\n");
		observed.promptTypeAfterShiftEnter = afterShiftEnter?.promptType ?? null;
	}
} catch (error) {
	failures.push(`probe threw: ${error instanceof Error ? error.message : error}`);
} finally {
	// Always restore the pasteboard, and always write the receipt — a thrown
	// step that skips the write leaves the PREVIOUS run's receipt on disk,
	// where it reads as fresh evidence.
	await writePasteboard(savedClipboard);
	if (d) await d.close();
}

const receipt: Record<string, Json> = {
	schemaVersion: 1,
	verifier: "flow-composer-multiline-probe",
	status: failures.length === 0 ? "green" : "red",
	observed: observed as unknown as Json,
	failures: failures as unknown as Json,
};
await writeFile(receiptPath, JSON.stringify(receipt, null, 2));
console.log(JSON.stringify(receipt, null, 2));
if (failures.length > 0) process.exit(1);

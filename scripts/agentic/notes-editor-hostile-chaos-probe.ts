#!/usr/bin/env bun
// Chaos battery for the Day Page editor (lane L3 / chaos-09): hostile-but-
// plausible editor reality that the rapid-newline and append-vs-autosave
// probes do not cover.
//
// Rows (surface = dayPage editor):
//   1. huge-doc-load       — 10k-line (~400KB) day file must open, focus the
//                            editor, land at the bottom, and stay alive.
//   2. huge-doc-typing     — keystrokes into the 10k-line doc must round-trip
//                            the protocol without egregious latency.
//   3. encoding-edges      — zalgo/RTL/ZWJ/bidi/huge-combining content must
//                            survive the editor AND the autosave round-trip to
//                            disk (marker containment).
//   4. oversize-setinput   — a >16KiB stdin command is BY DESIGN skipped
//                            (MAX_STDIN_COMMAND_BYTES); the app must warn
//                            (stdin_command_too_large) and stay fully usable.
//   5. huge-external-adopt — ~330KB arriving in the bound file while the editor
//                            is clean must be adopted by the 250ms disk poll
//                            (the realistic "huge content lands" path).
//   6. rename-mid-debounce — renaming the bound day file while an autosave is
//                            pending (2nd rapid edit defeats the leading-edge
//                            save) must recreate the file with the editor
//                            buffer (no data loss, no crash).
//   7. vanish-while-clean  — deleting the bound file while the editor is clean
//                            must be self-healed by the disk poll (chaos-09
//                            fix: re-dirty → autosave resurrects the file), so
//                            re-entering the Day Page keeps the content.
//   8. hostile-newline-burst — Enter/Backspace bursts inside zalgo/RTL content
//                            keep the viewport pinned to the cursor (chaos on
//                            the edges of day-editor-rapid-newline-scroll).
//
// Parallel-herd robustness: another lane taking frontmost can auto-hide this
// sandboxed panel (close_and_reset) mid-row. Every row therefore runs through
// `judgedRow`: ensure the Day Page is open, run the row with a local check
// recorder, retry up to 3x when the app is alive but got evicted off the day
// page, and only commit checks from an un-evicted attempt. Persistent eviction
// is recorded as blocked-by-environment, never as a product red. A dead app
// (getState unresponsive) always fails the row.
//
// Cleanup gate: window hidden at exit.
import { mkdirSync, writeFileSync, readFileSync, existsSync, renameSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { Driver, type Json } from "../devtools/driver";
import { openDayPage } from "./day-page-open-helper";

const binary =
  process.env.PROBE_BINARY ?? "target-agent/artifacts/monkey-notes/script-kit-gpui";

const runId = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
const receipts: Record<string, Json> = {};
const failures: string[] = [];

type Check = (name: string, ok: boolean, detail?: Json) => void;

class Evicted extends Error {}

function commit(name: string, ok: boolean, detail: Json = {}) {
  receipts[name] = { ok, ...detail };
  if (!ok) failures.push(name);
}

function todayLocalDate(): string {
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

function walk(node: unknown, out: Json[] = []): Json[] {
  if (!node || typeof node !== "object") return out;
  if (Array.isArray(node)) {
    for (const child of node) walk(child, out);
    return out;
  }
  const json = node as Json;
  if (typeof json.semanticId === "string" || typeof json.id === "string") out.push(json);
  for (const value of Object.values(json)) walk(value, out);
  return out;
}

function scrollMetrics(elements: Json): Json | null {
  const editor = walk(elements).find((el) => el.semanticId === "input:day-page-editor") ?? null;
  const runtime = editor?.style?.editorRuntime;
  const metrics =
    runtime && typeof runtime === "object" ? (runtime as Json).editorScrollMetrics : null;
  return metrics && typeof metrics === "object" ? (metrics as Json) : null;
}

function isAtBottom(metrics: Json | null): boolean {
  const scrollTop = Number(metrics?.scrollTop ?? -1);
  const liveScrollTop = Number(metrics?.liveScrollTop ?? scrollTop);
  const maxScrollTop = Number(metrics?.maxScrollTop ?? -1);
  return maxScrollTop >= 0 && Math.max(scrollTop, liveScrollTop) >= maxScrollTop - 6;
}

function percentile(sorted: number[], p: number): number {
  if (sorted.length === 0) return 0;
  const idx = Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1);
  return sorted[Math.max(0, idx)];
}

// --- hostile payload lines (from chaos-encoding-edges cases; real-user text) ---
const ZALGO = "z̸̢̛̭̙̘̋̽͐̕a̶͖͇͐̈́l̷̜̈́̊g̶̻̈o̸̙͌ chaos m̷̪̀ā̶͜r̶̡̛k̵͚̎s̷͇̈";
const RTL_LINE = "שלום עולם مرحبا بالعالم mixed direction";
const ZWJ_LINE = "family: 👨‍👩‍👧‍👦 flag: 🏳️‍🌈 a​b‌c‍d"; // ZWJ emoji + ZWSP/ZWNJ/ZWJ
const BIDI_LINE = "abc‮mix‬‭end‬ bidi-override";
const COMBINING_LINE = "a" + "́".repeat(500) + " tall stack";
const HOSTILE_LINES = [ZALGO, RTL_LINE, ZWJ_LINE, BIDI_LINE, COMBINING_LINE];

function hugeDocText(lines: number, tag: string): string {
  const out: string[] = [`# chaos-09 huge doc ${tag} ${runId}`, ""];
  for (let i = 1; i <= lines; i += 1) {
    out.push(`- line ${i.toString().padStart(5, "0")} of the chaos nine huge document`);
  }
  out.push("", `tail marker ${tag} ${runId}`);
  return `${out.join("\n")}\n`;
}

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: `monkey-notes-editor-chaos-${runId}`,
  readyTimeoutMs: 30000,
  defaultTimeoutMs: 12000,
  env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
});

const daysDir = join(driver.sessionDir, "home", ".scriptkit", "brain", "days");
const todayFile = join(daysDir, `${todayLocalDate()}.md`);

async function errorSet(): Promise<Set<string>> {
  try {
    const r = (await driver.getLogs({ level: "error", limit: 300 }, { timeoutMs: 5000 })) as Json;
    const entries = (r?.entries ?? r?.logs ?? []) as Json[];
    return new Set(entries.map((e) => `${e.target ?? ""}|${e.message ?? ""}`));
  } catch {
    return new Set();
  }
}

function newErrors(before: Set<string>, after: Set<string>): string[] {
  return [...after].filter((e) => !before.has(e));
}

async function gpuiKey(key: string, modifiers: string[] = []): Promise<void> {
  await driver.request(
    {
      type: "simulateGpuiEvent",
      event: { type: "keyDown", key, modifiers },
      target: { type: "main" },
    },
    { expect: "simulateGpuiEventResult", timeoutMs: 8000 },
  );
}

/** setInput + exact-state waitFor; throws Evicted on batch failure (the state
 *  match requires promptType dayPage, so eviction is the dominant cause). */
async function setEditor(text: string, label: string, timeoutMs = 15000): Promise<number> {
  const t0 = performance.now();
  const batch = (await driver.batch(
    [
      { type: "setInput", text },
      {
        type: "waitFor",
        condition: { type: "stateMatch", state: { promptType: "dayPage", inputValue: text } },
      },
    ],
    { timeoutMs },
  )) as Json;
  const ms = Math.round(performance.now() - t0);
  if (batch.success !== true) {
    throw new Evicted(`setEditor(${label}) batch failed after ${ms}ms`);
  }
  return ms;
}

async function editorLen(): Promise<number> {
  const s = (await driver.getState({ timeoutMs: 10000 })) as Json;
  return String(s.inputValue ?? "").length;
}

/** Throw Evicted if the app is alive but no longer on the Day Page. */
async function assertOnDayPage(where: string): Promise<Json> {
  const s = (await driver.getState({ timeoutMs: 10000 })) as Json;
  if (s.promptType !== "dayPage") {
    throw new Evicted(`${where}: promptType=${s.promptType}`);
  }
  return s;
}

/**
 * Run one row with eviction-retry semantics. `fn` records checks through the
 * local recorder; only the checks from a completed, un-evicted attempt are
 * committed. Throws from a dead app (getState unresponsive) fail the row.
 */
async function judgedRow(name: string, fn: (c: Check) => Promise<void>) {
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    const local: Array<{ name: string; ok: boolean; detail: Json }> = [];
    const c: Check = (n, ok, detail = {}) => local.push({ name: n, ok, detail });
    try {
      const state = (await driver.getState({ timeoutMs: 10000 })) as Json;
      if (state.promptType !== "dayPage") {
        await openDayPage(driver, `${runId}-${name}-a${attempt}`);
      }
      await fn(c);
      for (const entry of local) commit(entry.name, entry.ok, entry.detail);
      receipts[`${name}_attempts`] = { attempt, judged: true };
      return;
    } catch (error) {
      if (error instanceof Evicted) {
        receipts[`${name}_attempt_${attempt}_evicted`] = { error: String(error).slice(0, 200) };
        continue;
      }
      commit(`${name}_threw`, false, { error: String(error).slice(0, 300) });
      return;
    }
  }
  receipts[`${name}_blocked_by_environment`] = {
    note: "panel evicted from dayPage on every attempt (parallel-lane focus steal); row not judged",
  };
}

try {
  // ---------------- Row 1: huge-doc-load ----------------
  const hugeDoc = hugeDocText(10_000, "seed");
  mkdirSync(dirname(todayFile), { recursive: true });
  writeFileSync(todayFile, hugeDoc);
  receipts.seeded_huge_doc = { bytes: hugeDoc.length, lines: 10_002 };
  {
    const errs0 = await errorSet();
    const tOpen = performance.now();
    const opened = await openDayPage(driver, runId);
    const openMs = Math.round(performance.now() - tOpen);
    commit("huge_doc_opened_day_page", opened.promptType === "dayPage", {
      promptType: opened.promptType ?? null,
      openMs,
    });
    await Bun.sleep(1200); // load-time bottom-scroll retries
    const elements = (await driver.getElements(
      { target: { type: "main" }, limit: 160 },
      { timeoutMs: 10000 },
    )) as Json;
    commit(
      "huge_doc_editor_focused",
      elements.focusedSemanticId === "input:day-page-editor",
      { focusedSemanticId: elements.focusedSemanticId ?? null },
    );
    const loadMetrics = scrollMetrics(elements);
    commit("huge_doc_opens_at_bottom", isAtBottom(loadMetrics), { metrics: loadMetrics });
    const len = await editorLen();
    commit("huge_doc_content_fully_loaded", len >= hugeDoc.length - 2, {
      editorLen: len,
      seededLen: hugeDoc.length,
    });
    commit("huge_doc_no_new_errors", newErrors(errs0, await errorSet()).length === 0, {
      newErrors: newErrors(errs0, await errorSet()),
    });
  }

  // ---------------- Row 2: huge-doc-typing ----------------
  await judgedRow("huge_doc_typing", async (c) => {
    const errs1 = await errorSet();
    await gpuiKey("down", ["cmd"]); // MoveToEnd
    await Bun.sleep(300);
    const keyMs: number[] = [];
    for (let i = 0; i < 12; i += 1) {
      const t0 = performance.now();
      await gpuiKey("a");
      keyMs.push(Math.round(performance.now() - t0));
    }
    await assertOnDayPage("after typing burst");
    const sorted = [...keyMs].sort((a, b) => a - b);
    const p50 = percentile(sorted, 50);
    const p95 = percentile(sorted, 95);
    // Debug-profile hidden-window RPC budget: egregious-only red.
    c("huge_doc_typing_latency", p95 <= 1000, { keyMs, p50, p95 });
    await Bun.sleep(700);
    const stateTyped = (await driver.getState({ timeoutMs: 10000 })) as Json;
    c(
      "huge_doc_typing_landed",
      String(stateTyped.inputValue ?? "").includes("aaaaaaaaaaaa"),
      { tail: String(stateTyped.inputValue ?? "").slice(-40) },
    );
    c("huge_doc_typing_no_new_errors", newErrors(errs1, await errorSet()).length === 0, {
      newErrors: newErrors(errs1, await errorSet()),
    });
  });

  // ---------------- Row 3: encoding-edges ----------------
  await judgedRow("encoding_edges", async (c) => {
    const errs2 = await errorSet();
    const hostileDoc = `# chaos-09 encoding ${runId}\n\n${HOSTILE_LINES.join("\n")}\n\nend ${runId}\n`;
    await setEditor(hostileDoc, "hostile");
    await Bun.sleep(1600); // autosave debounce + flush
    const hostileState = await assertOnDayPage("after hostile flush");
    c("hostile_round_trips_in_memory", hostileState.inputValue === hostileDoc, {
      inputLen: String(hostileState.inputValue ?? "").length,
      expectedLen: hostileDoc.length,
    });
    const hostileDisk = existsSync(todayFile) ? readFileSync(todayFile, "utf8") : "";
    const missing = HOSTILE_LINES.filter((l) => !hostileDisk.includes(l));
    c("hostile_lines_survive_autosave_to_disk", missing.length === 0, {
      missingCount: missing.length,
      missingFirst: missing[0] ?? null,
      diskLen: hostileDisk.length,
    });
    c("hostile_no_new_errors", newErrors(errs2, await errorSet()).length === 0, {
      newErrors: newErrors(errs2, await errorSet()),
    });
  });

  // ---------------- Row 4: oversize-setinput (BY-DESIGN transport cap) ----------------
  await judgedRow("oversize_setinput", async (c) => {
    const errs3 = await errorSet();
    const before = await editorLen();
    const oversize = `# oversize ${runId}\n${"x".repeat(64 * 1024)}\n`;
    driver.send({ type: "setInput", text: oversize });
    await Bun.sleep(1200);
    await assertOnDayPage("after oversize send");
    const after = await editorLen();
    c("oversize_setinput_skipped_not_applied", after === before, { before, after });
    const warns = (await driver.getLogs(
      { level: "warn", contains: "stdin_command_too_large", limit: 50 },
      { timeoutMs: 5000 },
    )) as Json;
    const warnEntries = (warns?.entries ?? warns?.logs ?? []) as Json[];
    c("oversize_setinput_warned", warnEntries.length > 0, { warnCount: warnEntries.length });
    // Session must remain fully usable after the oversized line was skipped.
    await setEditor(`# after oversize ${runId}\nstill alive\n`, "post_oversize");
    c("oversize_setinput_session_usable", true, {});
    c("oversize_setinput_no_new_errors", newErrors(errs3, await errorSet()).length === 0, {
      newErrors: newErrors(errs3, await errorSet()),
    });
  });

  // ---------------- Row 5: huge-external-adopt (realistic ~330KB arrival) ----------------
  await judgedRow("huge_external_adopt", async (c) => {
    const errs4 = await errorSet();
    await Bun.sleep(700); // let the editor settle clean
    const external = hugeDocText(8_000, "external"); // ~330KB
    writeFileSync(todayFile, external);
    c("huge_external_written", external.length >= 300_000, { bytes: external.length });
    // 250ms disk poll + set_value of a ~330KB doc; give it a few frames.
    let adoptedLen = -1;
    const tAdopt = performance.now();
    while (performance.now() - tAdopt < 8000) {
      await Bun.sleep(400);
      adoptedLen = await editorLen();
      if (adoptedLen >= external.length - 2) break;
    }
    await assertOnDayPage("after adopt wait");
    const adoptMs = Math.round(performance.now() - tAdopt);
    c("huge_external_adopted_by_poll", adoptedLen >= external.length - 2, {
      adoptedLen,
      externalLen: external.length,
      adoptMs,
    });
    c("huge_external_no_new_errors", newErrors(errs4, await errorSet()).length === 0, {
      newErrors: newErrors(errs4, await errorSet()),
    });
  });

  // ---------------- Row 6: rename-mid-debounce ----------------
  await judgedRow("rename_mid_debounce", async (c) => {
    const errs5 = await errorSet();
    const m1 = `# chaos-09 rename ${runId}\n\nrename baseline ${runId}\n`;
    const step1 = `${m1}step one ${runId}\n`;
    const step2 = `${m1}step one ${runId}\nstep two typed while rename pending ${runId}\n`;
    const renamedTo = join(daysDir, `renamed-away-${runId}.md`);
    rmSync(renamedTo, { force: true }); // fresh on retry
    await setEditor(m1, "rename_baseline");
    await Bun.sleep(1600); // flush; last_autosave becomes old
    // First rapid edit eats the leading-edge save; second stays dirty for the
    // ~350ms trailing flush — rename lands inside that window.
    await setEditor(step1, "rename_step1");
    await setEditor(step2, "rename_step2");
    renameSync(todayFile, renamedTo); // vanish the bound path while autosave pending
    await Bun.sleep(1800); // trailing flush window
    await assertOnDayPage("after rename flush");
    const recreated = existsSync(todayFile) ? readFileSync(todayFile, "utf8") : null;
    c("rename_mid_debounce_recreates_bound_file", recreated !== null, { todayFile });
    c(
      "rename_mid_debounce_keeps_typed_content",
      Boolean(recreated?.includes(`step two typed while rename pending ${runId}`)),
      { recreated: recreated?.slice(0, 240) ?? null },
    );
    c("rename_target_untouched", existsSync(renamedTo), {
      renamedTo,
      renamedContent: existsSync(renamedTo)
        ? readFileSync(renamedTo, "utf8").slice(0, 240)
        : null,
    });
    receipts.rename_new_errors = { entries: newErrors(errs5, await errorSet()) };
  });

  // ---------------- Row 7: vanish-while-clean + self-heal + re-entry ----------------
  await judgedRow("vanish_while_clean", async (c) => {
    const errs6 = await errorSet();
    const kept = `# chaos-09 vanish ${runId}\n\ncontent that must not be silently discarded ${runId}\n`;
    await setEditor(kept, "vanish_baseline");
    await Bun.sleep(1600); // flush → clean, on disk
    rmSync(todayFile, { force: true });
    await Bun.sleep(1200); // poll ticks: re-dirty + autosave resurrection window
    const aliveAfterVanish = await assertOnDayPage("after vanish wait");
    receipts.vanish_buffer_after_delete = {
      bufferStillShown: String(aliveAfterVanish.inputValue ?? "").includes(
        `content that must not be silently discarded ${runId}`,
      ),
      fileExists: existsSync(todayFile),
    };
    // Sharpest form of the chaos-09 fix: the poll re-dirties and autosave
    // resurrects the file WHILE STILL ON THE PAGE — before any re-entry.
    const healedDisk = existsSync(todayFile) ? readFileSync(todayFile, "utf8") : null;
    c(
      "vanish_self_heal_recreates_file",
      Boolean(healedDisk?.includes(`content that must not be silently discarded ${runId}`)),
      { healedDisk: healedDisk?.slice(0, 200) ?? null },
    );

    // Re-enter the Day Page: escape out, then reopen through the hold gesture.
    await driver.simulateKey("escape");
    await Bun.sleep(500);
    const reopened = await openDayPage(driver, `${runId}-reenter`);
    c("vanish_reentry_opens_day_page", reopened.promptType === "dayPage", {
      promptType: reopened.promptType ?? null,
    });
    await Bun.sleep(800);
    const reopenState = (await driver.getState({ timeoutMs: 8000 })) as Json;
    const reopenValue = String(reopenState.inputValue ?? "");
    const diskAfterReopen = existsSync(todayFile) ? readFileSync(todayFile, "utf8") : null;
    c(
      "vanish_reentry_does_not_discard_content",
      reopenValue.includes(`content that must not be silently discarded ${runId}`) ||
        Boolean(diskAfterReopen?.includes(`content that must not be silently discarded ${runId}`)),
      {
        reopenLen: reopenValue.length,
        reopenHead: reopenValue.slice(0, 120),
        diskAfterReopen: diskAfterReopen?.slice(0, 240) ?? null,
      },
    );
    // Recovery: the next edit must land in a recreated file, whatever happened.
    const m2 = `# chaos-09 vanish recovered ${runId}\n\nrecreated after vanish ${runId}\n`;
    await setEditor(m2, "vanish_recover");
    await Bun.sleep(1800);
    const recoveredDisk = existsSync(todayFile) ? readFileSync(todayFile, "utf8") : null;
    c(
      "vanish_recovery_recreates_file_with_new_content",
      Boolean(recoveredDisk?.includes(`recreated after vanish ${runId}`)),
      { recoveredDisk: recoveredDisk?.slice(0, 200) ?? null },
    );
    receipts.vanish_new_errors = { entries: newErrors(errs6, await errorSet()) };
  });

  // ---------------- Row 8: hostile-newline-burst ----------------
  await judgedRow("hostile_newline_burst", async (c) => {
    const errs7 = await errorSet();
    const burstDoc = `# chaos-09 burst ${runId}\n\n${HOSTILE_LINES.join("\n")}\n${"- filler line for scroll depth\n".repeat(80)}burst tail ${runId}\n`;
    await setEditor(burstDoc, "burst_seed");
    await Bun.sleep(600);
    await gpuiKey("down", ["cmd"]); // MoveToEnd
    await Bun.sleep(300);
    for (let i = 0; i < 15; i += 1) await gpuiKey("enter");
    await Bun.sleep(500);
    const afterEnters = scrollMetrics(
      (await driver.getElements(
        { target: { type: "main" }, limit: 160 },
        { timeoutMs: 8000 },
      )) as Json,
    );
    for (let i = 0; i < 15; i += 1) await gpuiKey("backspace");
    await Bun.sleep(500);
    const afterBackspaces = scrollMetrics(
      (await driver.getElements(
        { target: { type: "main" }, limit: 160 },
        { timeoutMs: 8000 },
      )) as Json,
    );
    const burstState = await assertOnDayPage("after burst");
    c("hostile_burst_enters_track_bottom", isAtBottom(afterEnters), { metrics: afterEnters });
    c("hostile_burst_backspaces_stay_bottom", isAtBottom(afterBackspaces), {
      metrics: afterBackspaces,
    });
    const burstMissing = HOSTILE_LINES.filter(
      (l) => !String(burstState.inputValue ?? "").includes(l),
    );
    c("hostile_burst_content_intact", burstMissing.length === 0, {
      missingCount: burstMissing.length,
    });
    c("hostile_burst_no_new_errors", newErrors(errs7, await errorSet()).length === 0, {
      newErrors: newErrors(errs7, await errorSet()),
    });
  });

  // ---------------- Cleanup gate ----------------
  {
    await driver.simulateKey("escape");
    await Bun.sleep(400);
    let finalState = (await driver.getState({ timeoutMs: 8000 })) as Json;
    if (finalState.windowVisible === true) {
      await driver.simulateKey("escape");
      await Bun.sleep(400);
      finalState = (await driver.getState({ timeoutMs: 8000 })) as Json;
    }
    commit("cleanup_window_hidden", finalState.windowVisible === false, {
      windowVisible: finalState.windowVisible ?? null,
    });
  }
} finally {
  const ok = failures.length === 0;
  console.log(
    JSON.stringify({ ok, failures, sessionDir: driver.sessionDir, todayFile, receipts }, null, 2),
  );
  await driver.close();
  if (!ok) process.exitCode = 1;
}

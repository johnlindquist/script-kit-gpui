#!/usr/bin/env bun
// Chaos battery for the Notes window canonical-file churn (lane L3 / chaos-15).
//
// The Notes window differs from the Day Page: the sqlite DB is the read
// authority (`get_note` reads content from the DB) and `brain/notes/<slug>.md`
// is a write-through canonical store (`write_canonical_note_file`), with an
// external-edit guard that preserves foreign disk edits as `.conflict-` copies
// (`guard_external_edit_before_write`). These rows chaos that contract:
//
//   1. save-creates-file       — typed content lands in a canonical brain file.
//   2. vanish-mid-edit         — deleting the canonical file while an edit is
//                                in flight must be healed by the next save
//                                (guard passes a missing path; writer recreates).
//   3. rename-while-clean      — renaming the file away, then editing again,
//                                must recreate the canonical file and leave the
//                                renamed copy untouched.
//   4. reopen-after-vanish     — with the file vanished while clean, the note
//                                content must survive re-entry (DB authority).
//   5. external-edit-conflict  — a foreign edit to the canonical file followed
//                                by an editor save must preserve the foreign
//                                version as a `.conflict-` copy while the
//                                editor content wins the canonical file.
//
// Protocol-only (no native input, no screenshots — screen belongs to L4).
// Error-log discipline: every row reports NEW error entries. Cleanup: close
// all drivers; main window stays hidden (notes is a separate window).
import { readFileSync, existsSync, renameSync, rmSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const binary =
  process.env.PROBE_BINARY ?? "target-agent/artifacts/monkey-notes/script-kit-gpui";

const runId = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
const target = { type: "kind", kind: "notes", index: 0 };
const receipts: Record<string, Json> = {};
const failures: string[] = [];

function check(name: string, ok: boolean, detail: Json = {}) {
  receipts[name] = { ok, ...detail };
  if (!ok) failures.push(name);
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

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: `monkey-notes-window-churn-${runId}`,
  readyTimeoutMs: 30000,
  defaultTimeoutMs: 12000,
  env: { SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1" },
});

const notesDir = join(driver.sessionDir, "home", ".scriptkit", "brain", "notes");

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

async function openNotes() {
  driver.send({ type: "openNotes", requestId: `${runId}-open-${Date.now()}` });
  await Bun.sleep(900);
}

async function editorValue(): Promise<string | null> {
  const result = (await driver.getElements({ target, limit: 180 }, { timeoutMs: 6000 })) as Json;
  const editor = walk(result).find(
    (el) => el.semanticId === "input:notes-editor" || el.id === "notes-editor",
  );
  return typeof editor?.value === "string" ? editor.value : null;
}

/** setInput to the notes editor; retries once through a fresh openNotes when
 *  the notes entity is not yet resolvable. */
async function setNotesText(text: string, label: string): Promise<void> {
  for (let attempt = 1; attempt <= 2; attempt += 1) {
    const batch = (await driver.request(
      {
        type: "batch",
        requestId: `${runId}-set-${label}-a${attempt}`,
        target,
        commands: [{ type: "setInput", text }],
        options: { stopOnError: true, timeout: 5000 },
      },
      { expect: "batchResult", timeoutMs: 8000 },
    )) as Json;
    if (batch.success === true) {
      receipts[`set_notes_${label}`] = { ok: true, attempt };
      return;
    }
    await openNotes();
  }
  throw new Error(`setNotesText(${label}) failed after retries`);
}

/** All canonical (non-conflict) note files under brain/notes. */
function noteFiles(): string[] {
  if (!existsSync(notesDir)) return [];
  return readdirSync(notesDir)
    .filter((f) => f.endsWith(".md"))
    .map((f) => join(notesDir, f));
}

function conflictFiles(): string[] {
  return noteFiles().filter((f) => f.includes(".conflict-"));
}

function fileContaining(marker: string): string | null {
  for (const f of noteFiles()) {
    try {
      if (readFileSync(f, "utf8").includes(marker)) return f;
    } catch {
      /* raced with a writer; skip */
    }
  }
  return null;
}

/** Wait until some canonical file contains the marker (autosave flush). */
async function waitForFileWith(marker: string, budgetMs = 6000): Promise<string | null> {
  const t0 = performance.now();
  while (performance.now() - t0 < budgetMs) {
    const hit = fileContaining(marker);
    if (hit) return hit;
    await Bun.sleep(300);
  }
  return null;
}

try {
  const errs0 = await errorSet();
  await openNotes();

  // ---------------- Row 1: save-creates-file ----------------
  const markerA = `chaos15 alpha ${runId}`;
  const contentA = `# Chaos15 Note ${runId}\n\n${markerA}\n`;
  await setNotesText(contentA, "row1");
  const fileA = await waitForFileWith(markerA);
  check("save_creates_canonical_file", fileA !== null, {
    fileA,
    notesDirListing: noteFiles(),
  });
  check("row1_no_new_errors", newErrors(errs0, await errorSet()).length === 0, {
    newErrors: newErrors(errs0, await errorSet()),
  });
  if (!fileA) throw new Error("no canonical file — cannot continue churn rows");

  // ---------------- Row 2: vanish-mid-edit ----------------
  const errs1 = await errorSet();
  const markerB = `chaos15 bravo ${runId}`;
  const contentB = `# Chaos15 Note ${runId}\n\n${markerA}\n${markerB}\n`;
  await setNotesText(contentB, "row2");
  rmSync(fileA, { force: true }); // vanish while the 300ms debounce is pending
  const fileB = await waitForFileWith(markerB);
  check("vanish_mid_edit_recreates_file", fileB !== null, { fileB });
  const valueAfterB = await editorValue();
  check("vanish_mid_edit_editor_intact", Boolean(valueAfterB?.includes(markerB)), {
    editorTail: valueAfterB?.slice(-80) ?? null,
  });
  check("row2_no_new_errors", newErrors(errs1, await errorSet()).length === 0, {
    newErrors: newErrors(errs1, await errorSet()),
  });

  // ---------------- Row 3: rename-while-clean ----------------
  const errs2 = await errorSet();
  await Bun.sleep(1200); // let row 2 settle clean
  const canonical = fileContaining(markerB);
  check("row3_precondition_canonical_exists", canonical !== null, { canonical });
  const renamedTo = join(notesDir, `renamed-away-${runId}.md.bak`);
  if (canonical) renameSync(canonical, renamedTo);
  const markerC = `chaos15 charlie ${runId}`;
  const contentC = `# Chaos15 Note ${runId}\n\n${markerA}\n${markerB}\n${markerC}\n`;
  await setNotesText(contentC, "row3");
  const fileC = await waitForFileWith(markerC);
  check("rename_while_clean_recreates_on_edit", fileC !== null, { fileC });
  check("rename_target_untouched", existsSync(renamedTo), {
    renamedTo,
    renamedHasB: existsSync(renamedTo)
      ? readFileSync(renamedTo, "utf8").includes(markerB)
      : null,
  });
  check("row3_no_new_errors", newErrors(errs2, await errorSet()).length === 0, {
    newErrors: newErrors(errs2, await errorSet()),
  });

  // ---------------- Row 4: reopen-after-vanish (DB authority) ----------------
  const errs3 = await errorSet();
  await Bun.sleep(1200); // settle clean
  const canonicalC = fileContaining(markerC);
  if (canonicalC) rmSync(canonicalC, { force: true });
  receipts.row4_deleted_canonical = { canonicalC };
  await Bun.sleep(600);
  // Re-entry: openNotes is a TOGGLE (open_notes_window_with_close_behavior
  // closes an existing window). Toggle OFF, then toggle ON for a genuine
  // close+reopen; the note content must survive — the DB is the read authority.
  await openNotes(); // toggle OFF
  await Bun.sleep(600);
  await openNotes(); // toggle ON (fresh window)
  let reopenedValue: string | null = null;
  const tReopen = performance.now();
  while (performance.now() - tReopen < 8000) {
    reopenedValue = await editorValue().catch(() => null);
    if (reopenedValue !== null) break;
    await Bun.sleep(400);
  }
  check("reopen_after_vanish_preserves_content", Boolean(reopenedValue?.includes(markerC)), {
    reopenedTail: reopenedValue?.slice(-120) ?? null,
  });
  check("row4_no_new_errors", newErrors(errs3, await errorSet()).length === 0, {
    newErrors: newErrors(errs3, await errorSet()),
  });

  // ---------------- Row 5: external-edit-conflict ----------------
  const errs4 = await errorSet();
  // Bring the canonical file back via an editor save so the hash guard has a
  // current baseline, then edit it externally.
  const markerD = `chaos15 delta ${runId}`;
  const contentD = `# Chaos15 Note ${runId}\n\n${markerA}\n${markerB}\n${markerC}\n${markerD}\n`;
  await setNotesText(contentD, "row5_seed");
  const fileD = await waitForFileWith(markerD);
  check("row5_precondition_canonical_back", fileD !== null, { fileD });
  const conflictsBefore = conflictFiles().length;
  const foreignMarker = `FOREIGN external edit ${runId}`;
  if (fileD) {
    const raw = readFileSync(fileD, "utf8");
    await Bun.file(fileD).write(raw.replace(markerD, foreignMarker));
  }
  const markerE = `chaos15 echo ${runId}`;
  const contentE = `${contentD}${markerE}\n`;
  await setNotesText(contentE, "row5_edit");
  await waitForFileWith(markerE);
  // Poll for the conflict copy (written during the same save).
  let conflictsAfter = conflictsBefore;
  let foreignPreserved = false;
  const tConf = performance.now();
  while (performance.now() - tConf < 5000) {
    const conflicts = conflictFiles();
    conflictsAfter = conflicts.length;
    foreignPreserved = conflicts.some((f) => {
      try {
        return readFileSync(f, "utf8").includes(foreignMarker);
      } catch {
        return false;
      }
    });
    if (foreignPreserved) break;
    await Bun.sleep(300);
  }
  check("external_edit_preserved_as_conflict_copy", foreignPreserved, {
    conflictsBefore,
    conflictsAfter,
    conflictFiles: conflictFiles(),
  });
  const finalCanonical = fileContaining(markerE);
  check("editor_content_wins_canonical_file", finalCanonical !== null, { finalCanonical });
  check("row5_no_new_errors", newErrors(errs4, await errorSet()).length === 0, {
    newErrors: newErrors(errs4, await errorSet()),
  });

  // ---------------- Cleanup gate ----------------
  const mainState = (await driver.getState({ timeoutMs: 8000 })) as Json;
  check("cleanup_main_window_hidden", mainState.windowVisible !== true, {
    windowVisible: mainState.windowVisible ?? null,
  });
} finally {
  const ok = failures.length === 0;
  console.log(
    JSON.stringify({ ok, failures, sessionDir: driver.sessionDir, notesDir, receipts }, null, 2),
  );
  await driver.close();
  if (!ok) process.exitCode = 1;
}

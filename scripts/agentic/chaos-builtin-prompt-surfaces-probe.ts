#!/usr/bin/env bun
/**
 * chaos-builtin-prompt-surfaces-probe.ts — L2 (monkey-prompts) battery 07.
 * Permanent regression gate for the non-clipboard builtin prompt surfaces:
 *   emoji picker · file search · app launcher · settings · theme chooser
 *
 * Rows per surface (ledger L2 contract):
 *   nominal      — triggerBuiltin reaches the surface (view name matches),
 *                  deterministic datasets render rows
 *   empty-state  — no-match filter drives visibleChoiceCount to 0 while
 *                  choiceCount keeps reporting the total dataset (protocol
 *                  truthfulness — locks the battery-07 themeChooser swap fix)
 *   hostile      — encoding-edge payloads (chaos-encoding-edges.ts set) fired
 *                  into the surface filter; round-trip + latency asserted
 *   CLS          — stable chrome (input/footer/header/hint) must not drift
 *                  > 1px while only list content changes
 *   hint-audit   — script_kit::prompt_chrome logs: zero
 *                  prompt_hint_contract_violation entries, and the emoji
 *                  picker must emit prompt_hint_audit is_universal=true
 *                  (battery-07 fix for ledger OF-3)
 *
 * Safe: sandboxHome, protocol-only, never submits a row (no process spawn,
 * no paste), hidden-window (no `show`), Escape-only navigation.
 */
import { join } from "node:path";
import { Driver } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(process.cwd(), "target-agent/artifacts/monkey-prompts/script-kit-gpui");

const EXPLORE = process.env.CHAOS_EXPLORE === "1";
const CLS_EPS = 1.0;
const HOSTILE_SLOW_MS = 2000;

type Bounds = { x: number; y: number; width: number; height: number };

const HOSTILE: [string, string][] = [
  ["null-bytes", "abc\x00def"],
  ["control-chars", "a\x01b\x07c\x1bd\x7fe"],
  ["bidi-override", "abc‮mix‬‭end‬"],
  ["zwj-emoji", "\u{1F469}‍\u{1F469}‍\u{1F467}‍\u{1F466} family"],
  ["huge-combining", "a" + "́".repeat(2000)],
  ["long-line", "L".repeat(5000)],
  ["rtl-arabic", "مرحبا search"],
];

interface Surface {
  id: string;
  trigger: string;
  view: string;
  /** Dataset is deterministic (bundled/system catalog) — nominal rows must
   *  render and choiceCount must stay > 0 under a no-match filter. */
  deterministicDataset: boolean;
}
const SURFACES: Surface[] = [
  { id: "emoji-picker", trigger: "emoji", view: "emojiPicker", deterministicDataset: true },
  { id: "file-search", trigger: "files", view: "fileSearch", deterministicDataset: false },
  { id: "app-launcher", trigger: "apps", view: "appLauncher", deterministicDataset: true },
  { id: "settings", trigger: "settings", view: "settings", deterministicDataset: true },
  { id: "theme-chooser", trigger: "choose-theme", view: "themeChooser", deterministicDataset: true },
];

const findings: any[] = [];
const rows: any[] = [];
const note = (sev: string, surface: string, row: string, detail: any) => {
  findings.push({ sev, surface, row, detail });
  console.error(`[${sev}] ${surface}/${row}: ${JSON.stringify(detail).slice(0, 300)}`);
};

const d = await Driver.launch({
  sandboxHome: true,
  binary: BINARY,
  sessionName: "monkey-prompts-b07",
});

function stableComps(info: any): Map<string, Bounds> {
  const raw = Array.isArray(info?.components) ? info.components : [];
  const m = new Map<string, Bounds>();
  for (const c of raw) {
    if (!c?.bounds || typeof c.bounds.y !== "number") continue;
    const hay = `${c.name ?? ""} ${c.type ?? ""}`.toLowerCase();
    if (["input", "search", "footer", "header", "toolbar", "hint"].some((h) => hay.includes(h))) {
      m.set(`${c.name}|${c.type}`, c.bounds);
    }
  }
  return m;
}
const drift = (a: Bounds, b: Bounds) =>
  Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y), Math.abs(a.height - b.height));

async function errorSet(): Promise<Set<string>> {
  try {
    const r = await d.getLogs({ level: "error", limit: 300 }, { timeoutMs: 5000 });
    const entries: any[] = (r as any)?.entries ?? [];
    return new Set(entries.map((e: any) => `${e.target ?? ""}|${e.message ?? ""}`));
  } catch {
    return new Set();
  }
}

// prompt_chrome audit lines are deduped per process and the log ring rotates,
// so harvest incrementally after every surface battery and merge.
const chromeAudits = new Map<string, any>();
async function harvestChromeLogs() {
  try {
    const r = await d.getLogs(
      { target: "script_kit::prompt_chrome", limit: 500 },
      { timeoutMs: 5000 },
    );
    for (const e of ((r as any)?.entries ?? []) as any[]) {
      const msg = String(e.message ?? "");
      if (/prompt_hint_audit|surface_prompt_hint_audit|prompt_hint_contract_violation|prompt_chrome_audit/.test(msg)) {
        chromeAudits.set(msg, { level: e.level, message: msg });
      }
    }
  } catch {}
}

async function resetToMain() {
  for (let i = 0; i < 4; i++) {
    d.simulateKey("escape");
    await Bun.sleep(60);
  }
  d.setFilter("");
  await Bun.sleep(120);
  d.send({ type: "triggerBuiltin", name: "mainList" });
  await Bun.sleep(200);
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
}

async function state(): Promise<any> {
  try {
    return await d.getState({ timeoutMs: 8000 });
  } catch (e) {
    return { __dead: String(e).slice(0, 150) };
  }
}

try {
  await Bun.sleep(300);
  await d.waitForSettle({ timeoutMs: 4000 }).catch(() => {});

  for (const s of SURFACES) {
    await resetToMain();
    const errBefore = await errorSet();
    const row: any = { surface: s.id };

    // --- nominal ---
    d.send({ type: "triggerBuiltin", name: s.trigger });
    await Bun.sleep(300);
    await d.waitForSettle({ timeoutMs: 4000 }).catch(() => {});
    const st0 = await state();
    row.nominal = {
      view: st0.promptType ?? null,
      choiceCount: st0.choiceCount ?? null,
      visibleChoiceCount: st0.visibleChoiceCount ?? null,
      dead: st0.__dead ?? null,
    };
    if (EXPLORE) console.error(`[explore] ${s.id} nominal: ${JSON.stringify(st0).slice(0, 400)}`);
    if (st0.__dead) note("FAIL", s.id, "nominal", { dead: st0.__dead });
    else if ((st0.promptType ?? "") !== s.view)
      note("FAIL", s.id, "nominal", { expectedView: s.view, got: st0.promptType });
    else if (s.deterministicDataset && !(st0.visibleChoiceCount > 0))
      note("FAIL", s.id, "nominal", { visibleChoiceCount: st0.visibleChoiceCount });

    // Audits are emitted once per process on first render and the log ring
    // (capacity ~500) rotates fast under battery churn — harvest immediately.
    await harvestChromeLogs();

    // --- CLS within surface: type/backspace, stable chrome must hold ---
    const seq = ["", "a", "ab", "abc", "ab", "a", ""];
    let prev: Map<string, Bounds> | null = null;
    let prevLabel = "init";
    const shifts: any[] = [];
    for (const q of seq) {
      d.setFilter(q);
      await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
      let info: any = null;
      try {
        info = await d.getLayoutInfo({}, { timeoutMs: 6000 });
      } catch {}
      const cur = stableComps(info);
      const label = `filter="${q}"`;
      if (prev) {
        for (const [k, pb] of prev) {
          const cb = cur.get(k);
          if (cb && drift(pb, cb) > CLS_EPS) {
            shifts.push({ comp: k, from: prevLabel, to: label, px: Number(drift(pb, cb).toFixed(2)) });
          }
        }
      }
      prev = cur;
      prevLabel = label;
    }
    row.cls = { shifts: shifts.length, detail: shifts.slice(0, 6) };
    if (shifts.length) note("FAIL", s.id, "cls", shifts.slice(0, 4));

    // --- empty state: filter-aware count must drop to 0; total must not ---
    d.setFilter("zzqq0011-nomatch-\u{1F47B}");
    await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
    const stE = await state();
    row.empty = {
      choiceCount: stE.choiceCount ?? null,
      visibleChoiceCount: stE.visibleChoiceCount ?? null,
      dead: stE.__dead ?? null,
    };
    if (stE.__dead) note("FAIL", s.id, "empty", { dead: stE.__dead });
    else {
      if (stE.visibleChoiceCount !== 0)
        note("FAIL", s.id, "empty", {
          why: "visibleChoiceCount must be 0 on a no-match filter",
          visibleChoiceCount: stE.visibleChoiceCount,
        });
      if (s.deterministicDataset && !(stE.choiceCount > 0))
        note("FAIL", s.id, "empty", {
          why: "choiceCount (total dataset) must survive the filter",
          choiceCount: stE.choiceCount,
        });
    }

    // --- hostile input ---
    const hostile: any[] = [];
    for (const [label, payload] of HOSTILE) {
      const t0 = performance.now();
      d.setFilter(payload);
      let ok = true;
      let stH: any = null;
      try {
        stH = await d.getState({ timeoutMs: 15000 });
      } catch {
        ok = false;
      }
      const ms = Math.round(performance.now() - t0);
      hostile.push({ label, ms, ok, inLen: (stH?.inputValue ?? "").length });
      if (!ok) note("FAIL", s.id, `hostile:${label}`, { ms });
      else if (ms > HOSTILE_SLOW_MS) note("SLOW", s.id, `hostile:${label}`, { ms });
      d.setFilter("");
      await Bun.sleep(80);
    }
    row.hostile = hostile;

    // --- new error logs attributable to this surface's battery ---
    const errAfter = await errorSet();
    const newErrors = [...errAfter].filter((e) => !errBefore.has(e));
    row.newErrors = newErrors;
    if (newErrors.length) note("ERRLOG", s.id, "errors", newErrors.slice(0, 5));

    await harvestChromeLogs();
    rows.push(row);
  }

  // --- hint audits ---
  const audits = [...chromeAudits.values()];
  const violations = audits.filter((e) => e.message.includes("prompt_hint_contract_violation"));
  if (violations.length)
    note("FAIL", "footer", "hint-audit", violations.map((v) => v.message.slice(0, 160)).slice(0, 6));
  const emojiAudit = audits.find(
    (e) => e.message.includes("prompt_hint_audit") && e.message.includes("surface=emoji_picker"),
  );
  if (!emojiAudit)
    note("FAIL", "emoji-picker", "hint-audit", {
      why: "no prompt_hint_audit for surface=emoji_picker (OF-3 receipt missing)",
    });
  else if (!emojiAudit.message.includes("is_universal=true"))
    note("FAIL", "emoji-picker", "hint-audit", { message: emojiAudit.message.slice(0, 200) });
  rows.push({
    surface: "__hint_audits__",
    count: audits.length,
    entries: audits.map((e) => e.message.slice(0, 200)),
  });
} finally {
  // Cleanup gate: hidden window, driver closed.
  for (let i = 0; i < 3; i++) {
    d.simulateKey("escape");
    await Bun.sleep(50);
  }
  d.send({ type: "hide" });
  await Bun.sleep(200);
  let windowVisible: any = "unknown";
  try {
    windowVisible = ((await d.getState({ timeoutMs: 4000 })) as any).windowVisible;
  } catch {}
  const fails = findings.filter((f) => f.sev === "FAIL");
  const verdict = fails.length ? "FAIL" : findings.length ? "SUSPECT" : "PASS";
  console.log(
    JSON.stringify({ verdict, windowVisible, findings, rows, binary: BINARY }, null, 2),
  );
  await d.close();
}

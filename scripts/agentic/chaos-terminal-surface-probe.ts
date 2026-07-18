#!/usr/bin/env bun
/**
 * chaos-terminal-surface-probe.ts — L2 (monkey-prompts) battery 16.
 * Chaos rows for the Quick Terminal surface (src/terminal/**, term prompt
 * renderer, terminal_history):
 *
 *   nominal        — triggerBuiltin quick-terminal → promptType quickTerminal,
 *                    shell prompt appears in getElements
 *   hostile input  — newline-free encoding-edge bytes into the PTY via
 *                    setInput (send_raw_input): NUL, BEL, zalgo, bidi,
 *                    2000 combining marks, 4KB line, RTL; Ctrl-C clears
 *   scrollback     — inert `seq 1 20000` fills scrollback; app stays
 *                    responsive; CLS: top-anchored chrome holds during output
 *   shell exit     — `exit` kills the shell: record the resulting state
 *                    (any non-crash is acceptable), then recover + reopen
 *   rapid          — open/escape ×8 without waiting
 *   hint audit     — script_kit::prompt_chrome harvested right after open:
 *                    violations FAIL; audit presence recorded
 *
 * SAFETY: sandboxHome; the PTY runs the sandbox's shell but the ONLY
 * return-terminated inputs are inert (`seq …`, `exit`). Hostile payloads
 * carry no \r/\n so nothing else ever executes. Hidden-window (no `show`);
 * hide→show only as stuck-view recovery.
 */
import { join } from "node:path";
import { Driver } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(process.cwd(), "target-agent/artifacts/monkey-prompts/script-kit-gpui");
const EXPLORE = process.env.CHAOS_EXPLORE === "1";
const CLS_EPS = 1.0;
const VIEW = "quickTerminal";

type Bounds = { x: number; y: number; width: number; height: number };

const HOSTILE: [string, string][] = [
  ["nul-bel", "abc\x00def\x07ghi"],
  ["zalgo", "ź̴̨̀ą́̀ĺ̀ǵò"],
  ["bidi", "abc‮mix‬‭end‬"],
  ["combining", "a" + "́".repeat(2000)],
  ["long-line", "L".repeat(4000)],
  ["rtl", "مرحبا terminal"],
  ["esc-seq", "\x1b[31mred\x1b[0m\x1b]0;title\x07"],
];

const findings: any[] = [];
const rows: any[] = [];
const note = (sev: string, row: string, detail: any) => {
  findings.push({ sev, surface: "quick-terminal", row, detail });
  console.error(`[${sev}] quick-terminal/${row}: ${JSON.stringify(detail).slice(0, 250)}`);
};

const d = await Driver.launch({
  sandboxHome: true,
  binary: BINARY,
  sessionName: "monkey-prompts-b16",
});

async function state(): Promise<any> {
  try {
    return await d.getState({ timeoutMs: 10000 });
  } catch (e) {
    return { __dead: String(e).slice(0, 150) };
  }
}

async function errorSet(): Promise<Set<string>> {
  try {
    const r = await d.getLogs({ level: "error", limit: 300 }, { timeoutMs: 5000 });
    return new Set(((r as any)?.entries ?? []).map((e: any) => `${e.target ?? ""}|${e.message ?? ""}`));
  } catch {
    return new Set();
  }
}

async function elementsText(): Promise<string> {
  try {
    const r: any = await d.getElements({}, { timeoutMs: 8000 });
    const list: any[] = r?.elements ?? [];
    return list.map((e) => `${e.title ?? ""} ${e.text ?? ""} ${e.value ?? ""}`).join("\n");
  } catch {
    return "__getElements_failed__";
  }
}

function stableComps(info: any): Map<string, Bounds> {
  const m = new Map<string, Bounds>();
  for (const c of (info?.components ?? []) as any[]) {
    if (!c?.bounds) continue;
    const hay = `${c.name ?? ""} ${c.type ?? ""}`.toLowerCase();
    if (hay.includes("footer") || hay.includes("spacer")) continue;
    if (["input", "header", "hint", "toolbar"].some((h) => hay.includes(h)))
      m.set(`${c.name}|${c.type}`, c.bounds);
  }
  return m;
}

const chromeAudits = new Map<string, any>();
async function harvestChrome() {
  try {
    const r = await d.getLogs({ target: "script_kit::prompt_chrome", limit: 500 }, { timeoutMs: 5000 });
    for (const e of ((r as any)?.entries ?? []) as any[]) {
      const msg = String(e.message ?? "");
      if (/prompt_hint_audit|surface_prompt_hint_audit|prompt_hint_contract_violation|prompt_chrome_audit|prompt_chrome_contract_violation/.test(msg))
        chromeAudits.set(msg, { level: e.level, message: msg });
    }
  } catch {}
}

async function recoverToIdle(): Promise<any> {
  for (let i = 0; i < 3; i++) {
    d.simulateKey("escape");
    await Bun.sleep(150);
    const s = await state();
    if (s.__dead) return s;
    if ((s.promptType ?? "none") !== VIEW) return s;
  }
  // Terminals may consume Escape by design. L4 holds the SCREEN claim, so a
  // hide->show reset is off-limits here; builtin views (unlike sessionless
  // prompts) accept the mainList trigger as a reset.
  d.send({ type: "triggerBuiltin", name: "mainList" });
  await Bun.sleep(300);
  return await state();
}

/** Terminal input MUST go through batch: bare stdin setInput is rejected
 *  ("Unsupported protocol message"); only the batch executor routes
 *  setInput to QuickTerminalView.send_raw_input. Returns batch success. */
async function typeRaw(text: string): Promise<boolean> {
  try {
    const r: any = await d.batch([{ type: "setInput", text }], { timeoutMs: 6000 });
    return r?.success === true;
  } catch {
    return false;
  }
}

async function openTerminal(): Promise<any> {
  d.send({ type: "triggerBuiltin", name: "quick-terminal" });
  await Bun.sleep(400);
  await d.waitForSettle({ timeoutMs: 4000 }).catch(() => {});
  return await state();
}

try {
  await Bun.sleep(300);
  await d.waitForSettle({ timeoutMs: 4000 }).catch(() => {});

  // ── nominal open ────────────────────────────────────────────────
  {
    const errBefore = await errorSet();
    const st = await openTerminal();
    const row: any = { row: "nominal", view: st.promptType ?? null };
    if (st.__dead) note("FAIL", "nominal", { dead: st.__dead });
    else if ((st.promptType ?? "") !== VIEW)
      note("FAIL", "nominal", { expectedView: VIEW, got: st.promptType });
    // wait for the shell to draw something
    let shellReady = false;
    for (let i = 0; i < 12 && !shellReady; i++) {
      const txt = await elementsText();
      shellReady = txt.trim().length > 0 && txt !== "__getElements_failed__";
      if (!shellReady) await Bun.sleep(250);
    }
    row.shellReady = shellReady;
    if (!shellReady) note("SUSPECT", "nominal:shell", { note: "no terminal content within 3s (may be env/PTY spawn lag)" });
    await harvestChrome();
    const newErrors = [...(await errorSet())].filter((e) => !errBefore.has(e));
    row.newErrors = newErrors;
    if (newErrors.length) note("ERRLOG", "nominal:errors", newErrors.slice(0, 4));
    rows.push(row);
    if (EXPLORE) console.error(`[explore] nominal: ${JSON.stringify(row).slice(0, 300)}`);
  }

  // ── hostile PTY input (newline-free) ────────────────────────────
  for (const [label, payload] of HOSTILE) {
    const errBefore = await errorSet();
    const t0 = performance.now();
    const delivered = await typeRaw(payload);
    await Bun.sleep(150);
    const st = await state();
    const ms = Math.round(performance.now() - t0);
    const row: any = { row: `hostile:${label}`, ms, delivered, view: st.promptType ?? null };
    if (!delivered) note("FAIL", `hostile:${label}`, { note: "batch setInput not delivered" });
    if (st.__dead) note("FAIL", `hostile:${label}`, { dead: st.__dead });
    else if ((st.promptType ?? "") !== VIEW)
      note("FAIL", `hostile:${label}`, { lostView: st.promptType });
    else if (ms > 2000) note("SLOW", `hostile:${label}`, { ms });
    // Ctrl-C to clear the (never-executed) line
    await typeRaw("\x03");
    await Bun.sleep(120);
    const newErrors = [...(await errorSet())].filter((e) => !errBefore.has(e));
    row.newErrors = newErrors;
    if (newErrors.length) note("ERRLOG", `hostile:${label}:errors`, newErrors.slice(0, 3));
    rows.push(row);
  }

  // ── huge scrollback (inert seq) + CLS during output ─────────────
  {
    const errBefore = await errorSet();
    let info: any = null;
    try { info = await d.getLayoutInfo({}, { timeoutMs: 6000 }); } catch {}
    const before = stableComps(info);
    const t0 = performance.now();
    const seqDelivered = await typeRaw("seq 1 20000\r"); // PTY ZLE executes on \r, not \n
    if (!seqDelivered) note("FAIL", "scrollback:deliver", { note: "batch setInput not delivered" });
    const shifts: any[] = [];
    let sawTail = false;
    for (let i = 0; i < 20; i++) {
      await Bun.sleep(400);
      const txt = await elementsText();
      if (txt.includes("20000")) { sawTail = true; break; }
    }
    const ms = Math.round(performance.now() - t0);
    try { info = await d.getLayoutInfo({}, { timeoutMs: 6000 }); } catch { info = null; }
    const after = stableComps(info);
    for (const [k, pb] of before) {
      const cb = after.get(k);
      if (!cb) continue;
      const dpx = Math.max(Math.abs(pb.x - cb.x), Math.abs(pb.y - cb.y), Math.abs(pb.height - cb.height));
      if (dpx > CLS_EPS) shifts.push({ comp: k, px: Number(dpx.toFixed(2)) });
    }
    const st = await state();
    const row: any = { row: "scrollback", sawTail, ms, cls: shifts.length, view: st.promptType ?? null };
    if (st.__dead) note("FAIL", "scrollback", { dead: st.__dead });
    else if (!sawTail) note("SUSPECT", "scrollback", { note: "seq tail not observed in 8s", ms });
    if (shifts.length) note("FAIL", "scrollback:cls", shifts.slice(0, 4));
    const newErrors = [...(await errorSet())].filter((e) => !errBefore.has(e));
    row.newErrors = newErrors;
    if (newErrors.length) note("ERRLOG", "scrollback:errors", newErrors.slice(0, 3));
    rows.push(row);
    if (EXPLORE) console.error(`[explore] scrollback: ${JSON.stringify(row).slice(0, 300)}`);
  }

  // ── shell exit: error/terminal-dead state must not crash the app ─
  {
    const errBefore = await errorSet();
    if (!(await typeRaw("exit\r"))) note("FAIL", "shell-exit:deliver", { note: "batch setInput not delivered" });
    await Bun.sleep(1200);
    const st = await state();
    const row: any = { row: "shell-exit", viewAfter: st.promptType ?? null, dead: st.__dead ?? null };
    if (st.__dead) note("FAIL", "shell-exit", { dead: st.__dead });
    // any surviving state is acceptable — record it, then prove reopen works
    const idle = await recoverToIdle();
    if (idle.__dead) note("FAIL", "shell-exit:recover", { dead: idle.__dead });
    const st2 = await openTerminal();
    row.reopenView = st2.promptType ?? null;
    if (st2.__dead || (st2.promptType ?? "") !== VIEW)
      note("FAIL", "shell-exit:reopen", { got: st2.promptType, dead: st2.__dead ?? null });
    const newErrors = [...(await errorSet())].filter((e) => !errBefore.has(e));
    row.newErrors = newErrors;
    if (newErrors.length) note("ERRLOG", "shell-exit:errors", newErrors.slice(0, 4));
    rows.push(row);
    if (EXPLORE) console.error(`[explore] shell-exit: ${JSON.stringify(row).slice(0, 300)}`);
  }

  // ── rapid open/escape ×8 ─────────────────────────────────────────
  {
    const errBefore = await errorSet();
    for (let i = 0; i < 8; i++) {
      d.send({ type: "triggerBuiltin", name: "quick-terminal" });
      await Bun.sleep(120);
      d.simulateKey("escape");
      await Bun.sleep(80);
    }
    await d.waitForSettle({ timeoutMs: 4000 }).catch(() => {});
    const st = await openTerminal();
    const row: any = { row: "rapid-open-close", finalView: st.promptType ?? null };
    if (st.__dead) note("FAIL", "rapid", { dead: st.__dead });
    else if ((st.promptType ?? "") !== VIEW) note("FAIL", "rapid:reopen", { got: st.promptType });
    const newErrors = [...(await errorSet())].filter((e) => !errBefore.has(e));
    row.newErrors = newErrors;
    if (newErrors.length) note("ERRLOG", "rapid:errors", newErrors.slice(0, 4));
    rows.push(row);
  }

  // ── hint audits ──────────────────────────────────────────────────
  await harvestChrome();
  const audits = [...chromeAudits.values()];
  const violations = audits.filter((e) =>
    e.message.includes("prompt_hint_contract_violation") ||
    e.message.includes("prompt_chrome_contract_violation"),
  );
  if (violations.length) note("FAIL", "hint-audit", violations.map((v) => v.message.slice(0, 160)).slice(0, 4));
  const termAudit = audits.find((e) => /term|quick/i.test(e.message));
  if (!termAudit)
    note("PAPERCUT", "hint-audit", { note: "no prompt_chrome audit mentioning the terminal surface harvested (OF-3-style receipt gap or ring rotation)" });
  rows.push({ row: "__audits__", count: audits.length, entries: audits.map((a) => a.message.slice(0, 180)) });
} finally {
  await recoverToIdle();
  d.send({ type: "hide" });
  await Bun.sleep(200);
  let windowVisible: any = "unknown";
  try {
    windowVisible = ((await d.getState({ timeoutMs: 4000 })) as any).windowVisible;
  } catch {}
  const fails = findings.filter((f) => f.sev === "FAIL");
  const verdict = fails.length ? "FAIL" : findings.length ? "SUSPECT" : "PASS";
  console.log(JSON.stringify({ verdict, windowVisible, findings, rows, binary: BINARY }, null, 2));
  await d.close();
}

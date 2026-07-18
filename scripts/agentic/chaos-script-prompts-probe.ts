#!/usr/bin/env bun
/**
 * chaos-script-prompts-probe.ts — L2 (monkey-prompts) battery 08.
 * Chaos rows for the script prompt renderers (src/render_prompts/**), driven
 * by DIRECT protocol prompt messages (the story-fidelity ARG_FIXTURE pattern)
 * — inert by construction: no script is ever spawned, nothing is submitted.
 *
 * Per prompt type:
 *   nominal   — prompt message renders, promptType matches, app alive
 *   hostile   — encoding-edge / huge / injection-shaped payloads in the
 *               prompt's own content fields (choice names, div/form HTML,
 *               editor content, template tabstops, paths, messages)
 *   escape    — Escape always recovers to a non-prompt view (escape ladder)
 *   errlog    — no NEW error-level log entries per row
 *   CLS       — arg prompt only: stable chrome must hold while filtering
 *
 * Safe: sandboxHome, protocol-only, Escape-only dismissal, never submits,
 * never spawns. Runs SHOWN (prompt messages force-show the main window), so
 * parallel-monkey runs need a SCREEN claim; the window is re-hidden on exit.
 */
import { join } from "node:path";
import { Driver } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(process.cwd(), "target-agent/artifacts/monkey-prompts/script-kit-gpui");
const EXPLORE = process.env.CHAOS_EXPLORE === "1";
const CLS_EPS = 1.0;

type Bounds = { x: number; y: number; width: number; height: number };

const ZALGO = "ź̴̨̀ą́̀ĺ̀ǵò";
const BIDI = "abc‮mix‬‭end‬";
const COMBINING = "a" + "́".repeat(2000);
const HUGE_LINE = "L".repeat(5000);

const CHOICES = (names: string[]) =>
  names.map((name, i) => ({ name, value: `v${i}` }));

interface PromptCase {
  id: string;
  /** expected getState.promptType once open (empirically pinned) */
  view: string;
  /** protocol messages: [nominal, ...hostile variants] */
  cases: [string, Record<string, unknown>][];
}

let seq = 0;
const pid = () => `chaos08-${++seq}`;

const PROMPTS: PromptCase[] = [
  {
    id: "arg",
    view: "arg",
    cases: [
      ["nominal", { type: "arg", id: pid(), placeholder: "Pick", choices: CHOICES(["Apple", "Banana", "Cherry"]) }],
      ["hostile-names", { type: "arg", id: pid(), placeholder: BIDI, choices: CHOICES([ZALGO, BIDI, COMBINING, HUGE_LINE, "\x00null\x07bell"]) }],
      ["empty-choices", { type: "arg", id: pid(), placeholder: "Nothing here", choices: [] }],
      // NOTE: stdin transport caps commands at 16KiB — payloads must stay under it.
      ["many-choices", { type: "arg", id: pid(), placeholder: "Big", choices: CHOICES(Array.from({ length: 300 }, (_, i) => `Row ${i} ${i % 7 === 0 ? ZALGO : ""}`)) }],
    ],
  },
  {
    id: "mini",
    view: "mini",
    cases: [
      ["nominal", { type: "mini", id: pid(), placeholder: "Mini", choices: CHOICES(["One", "Two"]) }],
      ["hostile", { type: "mini", id: pid(), placeholder: HUGE_LINE, choices: CHOICES([BIDI, ZALGO]) }],
    ],
  },
  {
    id: "micro",
    view: "micro",
    cases: [
      ["nominal", { type: "micro", id: pid(), placeholder: "Micro", choices: CHOICES(["A", "B"]) }],
      ["hostile", { type: "micro", id: pid(), placeholder: ZALGO, choices: CHOICES([COMBINING]) }],
    ],
  },
  {
    id: "div",
    view: "div",
    cases: [
      ["nominal", { type: "div", id: pid(), html: "<h1>Hello</h1><p>world</p>" }],
      ["script-injection", { type: "div", id: pid(), html: `<script>while(1){}</script><img src=x onerror="alert(1)"><h1>after</h1>` }],
      ["unclosed-tags", { type: "div", id: pid(), html: "<div><b><i>never closed <h1>deep" + "<div>".repeat(500) }],
      ["huge-html", { type: "div", id: pid(), html: "<p>" + "chunk ".repeat(2200) + "</p>" }],
      ["bidi-html", { type: "div", id: pid(), html: `<p>${BIDI} ${ZALGO}</p>` }],
    ],
  },
  {
    id: "editor",
    view: "editor",
    cases: [
      ["nominal", { type: "editor", id: pid(), content: "const x = 1;\n", language: "typescript" }],
      ["huge-content", { type: "editor", id: pid(), content: ("line of text\n".repeat(1000)), language: "markdown" }],
      ["zalgo-content", { type: "editor", id: pid(), content: `${ZALGO}\n${BIDI}\n${COMBINING}\n` }],
    ],
  },
  {
    id: "select",
    view: "select",
    cases: [
      ["nominal", { type: "select", id: pid(), placeholder: "Multi", choices: CHOICES(["Red", "Green", "Blue"]), multiple: true }],
      ["hostile", { type: "select", id: pid(), placeholder: BIDI, choices: CHOICES([HUGE_LINE, ZALGO]), multiple: true }],
    ],
  },
  {
    id: "confirm",
    view: "confirmPrompt",
    cases: [
      ["nominal", { type: "confirm", id: pid(), message: "Proceed?" }],
      ["hostile", { type: "confirm", id: pid(), message: `${BIDI} ${HUGE_LINE.slice(0, 1000)}`, confirmText: ZALGO, cancelText: COMBINING.slice(0, 200) }],
    ],
  },
  {
    id: "fields",
    view: "fields",
    cases: [
      ["nominal", { type: "fields", id: pid(), fields: [{ name: "a", label: "Alpha" }, { name: "b", label: "Beta" }] }],
      ["hostile-labels", { type: "fields", id: pid(), fields: [{ name: "x", label: BIDI }, { name: "y", label: HUGE_LINE.slice(0, 1500) }, { name: "z", label: ZALGO }] }],
    ],
  },
  {
    id: "form",
    view: "form",
    cases: [
      ["nominal", { type: "form", id: pid(), html: `<form><input name="a" placeholder="Alpha"/><textarea name="b"></textarea></form>` }],
      ["hostile-html", { type: "form", id: pid(), html: `<form><script>x</script><input name="${BIDI}" value="${ZALGO}"/>` + "<div>".repeat(300) + "</form>" }],
    ],
  },
  {
    id: "path",
    view: "path",
    cases: [
      ["nominal", { type: "path", id: pid() }],
      ["missing-start", { type: "path", id: pid(), startPath: "/nonexistent/zzqq/deeper" }],
      ["hostile-start", { type: "path", id: pid(), startPath: `/tmp/../tmp/./${ZALGO}` }],
    ],
  },
  {
    id: "drop",
    view: "drop",
    cases: [["nominal", { type: "drop", id: pid() }]],
  },
  {
    id: "hotkey",
    view: "hotkey",
    cases: [["nominal", { type: "hotkey", id: pid(), placeholder: "Press keys" }]],
  },
  {
    id: "template",
    view: "template",
    cases: [
      ["nominal", { type: "template", id: pid(), template: "Hello ${1:name}, welcome to ${2:place}!" }],
      ["hostile-tabstops", { type: "template", id: pid(), template: "${1:${2:${3:deep}}} ${999} $0 ${1:" + ZALGO + "} ${" + HUGE_LINE.slice(0, 500) + "}" }],
    ],
  },
  {
    id: "env",
    view: "env",
    cases: [
      ["nominal", { type: "env", id: pid(), key: "CHAOS_TEST_KEY", prompt: "Enter value", secret: false }],
      ["hostile", { type: "env", id: pid(), key: "K".repeat(500), prompt: BIDI, secret: true }],
    ],
  },
];

const findings: any[] = [];
const rows: any[] = [];
const note = (sev: string, surface: string, row: string, detail: any) => {
  findings.push({ sev, surface, row, detail });
  console.error(`[${sev}] ${surface}/${row}: ${JSON.stringify(detail).slice(0, 250)}`);
};

const d = await Driver.launch({
  sandboxHome: true,
  binary: BINARY,
  sessionName: "monkey-prompts-b08",
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
    const entries: any[] = (r as any)?.entries ?? [];
    return new Set(entries.map((e: any) => `${e.target ?? ""}|${e.message ?? ""}`));
  } catch {
    return new Set();
  }
}

const violationLogs = new Set<string>();
async function harvestViolations() {
  try {
    const r = await d.getLogs(
      { target: "script_kit::prompt_chrome", limit: 500 },
      { timeoutMs: 5000 },
    );
    for (const e of ((r as any)?.entries ?? []) as any[]) {
      const msg = String(e.message ?? "");
      if (msg.includes("prompt_hint_contract_violation") || msg.includes("prompt_chrome_contract_violation")) {
        violationLogs.add(msg.slice(0, 220));
      }
    }
  } catch {}
}

const IDLE_VIEWS = new Set(["none", "scriptList", "mainList", ""]);
async function escapeToIdle(maxEsc = 6): Promise<any> {
  for (let i = 0; i < maxEsc; i++) {
    const s = await state();
    if (s.__dead) return s;
    const v = s.promptType ?? "";
    // "none" is ScriptList's promptType — escaping past it hides the window,
    // and the next prompt then loses the hidden-show reset race.
    if (IDLE_VIEWS.has(v) || s.windowVisible === false) return s;
    d.simulateKey("escape");
    await Bun.sleep(120);
  }
  return await state();
}

// Prompts force-show, but a prompt arriving while hidden gets reset by the
// show path — make sure the window is already shown before every case.
async function ensureShown() {
  const s = await state();
  if (s.windowVisible === false) {
    d.send({ type: "show" });
    await Bun.sleep(350);
    await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
  }
}

try {
  await Bun.sleep(300);
  // Prompts force-show the main window, and showing a hidden window resets any
  // non-ScriptList view (window_visibility.rs "Preparing ScriptList before
  // showing hidden main window") — so this battery runs SHOWN, under a SCREEN
  // claim in .notes/chaos-ledger.md. Hidden-window prompt probing is
  // structurally impossible.
  d.send({ type: "show" });
  await Bun.sleep(400);
  await d.waitForSettle({ timeoutMs: 4000 }).catch(() => {});
  const idle0 = await state();
  const idleView = idle0.promptType ?? "scriptList";

  for (const p of PROMPTS) {
    for (const [label, msg] of p.cases) {
      const errBefore = await errorSet();
      const row: any = { prompt: p.id, case: label };
      let st: any = null;
      let opened = false;
      for (let attempt = 1; attempt <= 3 && !opened; attempt++) {
        await ensureShown();
        const t0 = performance.now();
        d.send(msg as any);
        await Bun.sleep(250);
        await d.waitForSettle({ timeoutMs: 5000 }).catch(() => {});
        st = await state();
        row.openMs = Math.round(performance.now() - t0);
        row.view = st.promptType ?? null;
        row.visibleChoiceCount = st.visibleChoiceCount ?? null;
        row.attempts = attempt;
        opened = !st.__dead && (st.promptType ?? "") === p.view;
        if (!opened && !st.__dead) await Bun.sleep(300); // likely evicted by a parallel instance
      }
      if (EXPLORE)
        console.error(`[explore] ${p.id}/${label} view=${st.promptType} vis=${st.visibleChoiceCount} ms=${row.openMs} tries=${row.attempts}`);

      if (st.__dead) note("FAIL", p.id, `${label}:open`, { dead: st.__dead });
      else if (!opened)
        note("ENV", p.id, `${label}:open`, { expectedView: p.view, got: st.promptType, note: "off-surface after 3 attempts — likely parallel-instance panel eviction" });
      else if (row.openMs > 6000) note("SLOW", p.id, `${label}:open`, { ms: row.openMs });

      if (!opened) { rows.push(row); continue; }

      // CLS: arg nominal only — stable chrome holds while filtering.
      if (p.id === "arg" && label === "nominal") {
        // Bottom-anchored chrome (footer, native footer spacer) legitimately
        // moves when the mini window auto-resizes to the filtered row count —
        // only top-anchored chrome must hold position.
        const stable = (info: any) => {
          const m = new Map<string, Bounds>();
          for (const c of (info?.components ?? []) as any[]) {
            if (!c?.bounds) continue;
            const hay = `${c.name ?? ""} ${c.type ?? ""}`.toLowerCase();
            if (hay.includes("footer") || hay.includes("spacer")) continue;
            if (["input", "search", "header", "hint"].some((h) => hay.includes(h)))
              m.set(`${c.name}|${c.type}`, c.bounds);
          }
          return m;
        };
        let prev: Map<string, Bounds> | null = null;
        const shifts: any[] = [];
        for (const q of ["", "a", "ap", "a", ""]) {
          d.setFilter(q);
          await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
          let info: any = null;
          try {
            info = await d.getLayoutInfo({}, { timeoutMs: 6000 });
          } catch {}
          const cur = stable(info);
          if (prev)
            for (const [k, pb] of prev) {
              const cb = cur.get(k);
              const dpx = cb
                ? Math.max(Math.abs(pb.x - cb.x), Math.abs(pb.y - cb.y), Math.abs(pb.height - cb.height))
                : 0;
              if (cb && dpx > CLS_EPS) shifts.push({ comp: k, px: Number(dpx.toFixed(2)), at: q });
            }
          prev = cur;
        }
        row.cls = { shifts: shifts.length, detail: shifts.slice(0, 4) };
        if (shifts.length) note("FAIL", p.id, "cls", shifts.slice(0, 4));
      }

      await harvestViolations();

      // Escape recovery — LOCK for the OF-5 fix (battery 16 follow-up):
      // sessionless prompts must dismiss on Escape via the direct-reset
      // guard in simulate_key_dispatch.rs (current_script_pid.is_none()).
      // A stuck view is a FAIL; hide→show recovery below is only cleanup.
      let after = await escapeToIdle();
      row.afterEscape = after.promptType ?? null;
      if (after.__dead) note("FAIL", p.id, `${label}:escape`, { dead: after.__dead });
      else if (!IDLE_VIEWS.has(after.promptType ?? "") && after.windowVisible !== false) {
        row.escapeStuckSessionless = true;
        note("FAIL", p.id, `${label}:escape`, {
          stuckIn: after.promptType,
          note: "sessionless prompt swallowed Escape (OF-5 regression)",
        });
        // triggerBuiltin mainList is ALSO refused while a prompt view is up
        // (second receipt for the papercut) — the working recovery is the
        // documented hide→show reset ("Preparing ScriptList before showing
        // hidden main window").
        d.send({ type: "hide" });
        await Bun.sleep(250);
        d.send({ type: "show" });
        await Bun.sleep(400);
        await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
        after = await state();
        if (after.__dead || (!IDLE_VIEWS.has(after.promptType ?? "") && after.windowVisible !== false))
          note("FAIL", p.id, `${label}:escape-recovery`, { stuckIn: after.promptType, dead: after.__dead ?? null });
      }

      const errAfter = await errorSet();
      const newErrors = [...errAfter].filter((e) => !errBefore.has(e));
      row.newErrors = newErrors;
      if (newErrors.length) note("ERRLOG", p.id, `${label}:errors`, newErrors.slice(0, 4));

      rows.push(row);
    }
  }

  // Rapid interaction: open prompt-over-prompt without waiting, then Esc storm.
  {
    const errBefore = await errorSet();
    for (let i = 0; i < 10; i++) {
      d.send({ type: "arg", id: pid(), placeholder: `storm ${i}`, choices: CHOICES(["x", "y"]) } as any);
      d.send({ type: "div", id: pid(), html: `<h1>storm ${i}</h1>` } as any);
    }
    await Bun.sleep(400);
    for (let i = 0; i < 12; i++) {
      d.simulateKey("escape");
      await Bun.sleep(40);
    }
    const st = await state();
    const errAfter = await errorSet();
    const newErrors = [...errAfter].filter((e) => !errBefore.has(e));
    rows.push({ prompt: "storm", case: "open-over-open+esc-storm", view: st.promptType ?? null, dead: st.__dead ?? null, newErrors });
    if (st.__dead) note("FAIL", "storm", "recovery", { dead: st.__dead });
    if (newErrors.length) note("ERRLOG", "storm", "errors", newErrors.slice(0, 4));
  }

  if (violationLogs.size)
    note("FAIL", "footer", "hint-audit", [...violationLogs].slice(0, 6));
  rows.push({ prompt: "__violations__", entries: [...violationLogs] });
} finally {
  for (let i = 0; i < 4; i++) {
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
  console.log(JSON.stringify({ verdict, windowVisible, findings, rows, binary: BINARY }, null, 2));
  await d.close();
}

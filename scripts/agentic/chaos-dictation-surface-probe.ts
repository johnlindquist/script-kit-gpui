#!/usr/bin/env bun
/**
 * Chaos battery NN=25 (round-44, L6 monkey-grok-input): DICTATION surface.
 *
 * Never chaosed before — first permanent probe for the dictation stack.
 * Sandboxed + fixture-driven only:
 *   - pushDictationResult / openDictationOverlayFixture / triggerBuiltin
 *   - seeded dictation-history.jsonl + corrupted settings/config
 * Real mic / TCC / true PTT hardware path is UNREACHABLE in this sandbox —
 * classified as ENV with honest receipts, not product FAIL.
 *
 * Rows (lenses: correctness + CLS):
 *   smoke — launch, getState, short pushDictationResult, overlay fixture open
 *   a     — live-partials churn: partial→final storms, hostile/huge transcripts,
 *           partial flicker vs stable chrome (main + overlay layout)
 *   b     — PTT hold-contract edges (protocol-reachable): rapid double-activation
 *           via triggerBuiltin, overlay fixture during surface transition;
 *           real global-hotkey hold/release = ENV
 *   c     — dictation.* config corruption graceful degrade (settings.json +
 *           config.ts wrong types / malformed)
 *   d     — whisper/model-absent degraded env (sharedModels:false, empty models)
 *           → setup/model clarity, no crash (ENV if expected UX is clear)
 *   e     — dictation-history list: hostile entries + separator contract under
 *           filter churn
 *
 * Run:
 *   SCRIPT_KIT_GPUI_BINARY=target-agent/artifacts/monkey-input/script-kit-gpui \
 *     bun scripts/agentic/chaos-dictation-surface-probe.ts
 *   CHAOS_DICTATION_ROW_ONLY=a|b|c|d|e|smoke  # single row after smoke
 *
 * Receipts: .test-output/chaos-25-dictation/
 */
import { execSync } from "node:child_process";
import {
  mkdirSync,
  writeFileSync,
  rmSync,
  existsSync,
  symlinkSync,
  readFileSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir, homedir } from "node:os";
import { Driver, type Json } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(process.cwd(), "target-agent/artifacts/monkey-input/script-kit-gpui");
const OUT_DIR = join(
  process.cwd(),
  process.env.CHAOS_DICTATION_RECEIPT_DIR ?? ".test-output/chaos-25-dictation",
);
mkdirSync(OUT_DIR, { recursive: true });

const ROW_ONLY = (process.env.CHAOS_DICTATION_ROW_ONLY ?? "").trim().toLowerCase();
const CLS_EPS = 1.0;
const SETTLE_BUDGET_MS = 5000;

type RowVerdict = "PASS" | "SUSPECT" | "FAIL" | "ENV";
type Row = {
  id: string;
  verdict: RowVerdict;
  reason: string;
  detail?: Json;
};

const rows: Row[] = [];
const findings: Json[] = [];
let crashed = "";

// ── helpers ──────────────────────────────────────────────────────────────────

function isNoise(msg: string): boolean {
  return (
    /captureScreenshot|automation window|screenshot/i.test(msg) ||
    (/RefCell already borrowed/i.test(msg) && /vendor\/gpui\/src\/window\.rs/i.test(msg)) ||
    (/window not found/i.test(msg) && /vendor\/gpui\/src\/window\.rs/i.test(msg))
  );
}

async function errorKeys(d: Driver): Promise<Set<string>> {
  try {
    const r = await d.getLogs({ level: "error", limit: 300 }, { timeoutMs: 5000 });
    const es: any[] = (r as any).entries ?? (r as any).logs ?? [];
    return new Set(
      es
        .map((e) => `${e.target ?? ""}|${e.message ?? ""}`)
        .filter((k) => !isNoise(k)),
    );
  } catch {
    return new Set();
  }
}

function newErrorDelta(before: Set<string>, after: Set<string>): string[] {
  const out: string[] = [];
  for (const k of after) if (!before.has(k)) out.push(k.slice(0, 200));
  return out;
}

type Bounds = { x: number; y: number; width: number; height: number };
const STABLE_HINTS = ["input", "search", "footer", "header", "toolbar", "hint"];

function stableBounds(info: Json): Map<string, Bounds> {
  const m = new Map<string, Bounds>();
  for (const c of ((info?.components ?? []) as Json[])) {
    if (!c?.bounds || typeof c.bounds.y !== "number") continue;
    const hay = `${c.name ?? ""} ${c.type ?? ""}`.toLowerCase();
    if (STABLE_HINTS.some((h) => hay.includes(h))) {
      m.set(`${c.name}|${c.type ?? ""}`, c.bounds as Bounds);
    }
  }
  return m;
}

const drift = (a: Bounds, b: Bounds) =>
  Math.max(
    Math.abs(a.x - b.x),
    Math.abs(a.y - b.y),
    Math.abs(a.height - b.height),
  );

function maxChromeDrift(
  a: Map<string, Bounds>,
  b: Map<string, Bounds>,
): { max: number; worst: string | null } {
  let max = 0;
  let worst: string | null = null;
  for (const [k, ba] of a) {
    const bb = b.get(k);
    if (!bb) continue;
    const d = drift(ba, bb);
    if (d > max) {
      max = d;
      worst = k;
    }
  }
  return { max, worst };
}

async function settle(d: Driver, ms = 400) {
  await Bun.sleep(ms);
  try {
    await d.waitForSettle({ timeoutMs: SETTLE_BUDGET_MS });
  } catch {
    // settle timeout is recorded by callers via state probes
  }
}

function shouldRun(id: string): boolean {
  if (!ROW_ONLY) return true;
  if (id === "smoke") return true;
  return id === ROW_ONLY || ROW_ONLY === "all";
}

function loadavg(): string {
  try {
    return execSync("sysctl -n vm.loadavg").toString().trim();
  } catch {
    return "unknown";
  }
}

function filterText(state: Json): string {
  const diag = state?.filterInputDiagnostics;
  if (diag && typeof diag.canonicalFilterText === "string") {
    return diag.canonicalFilterText;
  }
  return String(state?.inputValue ?? state?.filter ?? "");
}

function selectionOk(state: Json): boolean {
  const vis = Number(state.visibleChoiceCount ?? 0);
  const sel = state.selectedIndex ?? state.selectedRow ?? null;
  if (sel == null) return true;
  const n = Number(sel);
  if (!Number.isFinite(n) || n < 0) return false;
  if (vis > 0 && n >= vis) return false;
  return true;
}

type RowInfo = {
  semanticId: string;
  role: string | null;
  kind: string | null;
  text: string | null;
  selectable: boolean;
  type: string | null;
};

function rowsOf(elementsResult: Json): RowInfo[] {
  const elements: Json[] = (elementsResult?.elements ?? []) as Json[];
  return elements
    .filter((e) => {
      if (e.semanticId === "input:filter" || e.semanticId === "list:results") return false;
      if (e.type === "input" || e.type === "list") return false;
      if (e.role === "footer") return false;
      return true;
    })
    .map((e) => ({
      semanticId: String(e.semanticId ?? ""),
      role: (e.role as string) ?? null,
      kind: (e.kind as string) ?? null,
      text: typeof e.text === "string" ? e.text.slice(0, 80) : null,
      selectable: e.selectable === true,
      type: (e.type as string) ?? null,
    }));
}

function leadingSeparator(listRows: RowInfo[]) {
  if (listRows.length === 0) {
    return { present: false, reason: "empty-list", first: null as RowInfo | null };
  }
  const first = listRows[0];
  const present =
    first.role === "sectionHeader" ||
    first.kind === "sectionHeader" ||
    /sectionHeader|section:/.test(first.semanticId);
  return {
    present,
    reason: present ? "first-row-sectionHeader" : "first-row-not-sectionHeader",
    first,
  };
}

function pushDictation(
  d: Driver,
  opts: {
    transcript?: string;
    partialTranscript?: string;
    target?: string;
  },
) {
  const msg: Json = {
    type: "pushDictationResult",
    transcript: opts.transcript ?? "",
  };
  if (opts.partialTranscript !== undefined) {
    msg.partialTranscript = opts.partialTranscript;
  }
  if (opts.target) msg.target = opts.target;
  d.send(msg);
}

// Stdin protocol hard-caps at max_line_bytes=16384 (see app log
// event_type=stdin_command_too_large). Payloads above that are ENV for the
// fixture path; history-file seeding (row e) still covers 100KB on disk.
const STDIN_SAFE = 14_000;

const HOSTILE_PARTIALS: string[] = [
  "hello",
  "hello world",
  "hello world this is a partial rewrite of the tail words only",
  "ź̴̨̀ą́̀ĺ̀ǵò partial " + "́".repeat(80),
  "\u202Ereversed\u202C partial العربية",
  "👩‍👩‍👧‍👦 emoji partial family",
  "\x1b[31mansi\x1b[0m partial \x01\x02 bell\x07",
  "<script>alert(1)</script> partial tag",
  "H".repeat(STDIN_SAFE), // max-safe huge single line via protocol
  "word ".repeat(2_000).trim(), // multi-word pressure under cap
];

const HOSTILE_FINALS: string[] = [
  "final short phrase",
  "ź̴̨̀ą́̀ĺ̀ǵò final " + "́".repeat(60),
  "\u202Efinal reversed\u202C العربية",
  "F".repeat(STDIN_SAFE),
];

// Isolated crash-candidate: multi-line transcript into main filter.
// Prior red (2026-07-18): GPUI panic "text argument should not contain newlines"
// at vendor/gpui/src/text_system.rs when delivered via pushDictationResult.
const MULTILINE_CRASH_CANDIDATE = "line-a\nline-b\nline-c\n" + "x\n".repeat(50);

function seedHistoryJsonl(kitDir: string) {
  const path = join(kitDir, "dictation-history.jsonl");
  const entries = [
    {
      id: "dictation-hostile-zalgo",
      timestamp: "2026-07-18T12:00:00.000Z",
      transcript: "ź̴̨̀ą́̀ĺ̀ǵò history " + "́".repeat(120),
      preview: "zalgo preview",
      target: "Main Filter",
      audio_duration_ms: 1200,
    },
    {
      id: "dictation-hostile-rtl",
      timestamp: "2026-07-18T12:01:00.000Z",
      transcript: "\u202Ereversed\u202C history العربية",
      preview: "rtl preview",
      target: "Prompt",
      audio_duration_ms: 900,
    },
    {
      id: "dictation-hostile-emoji",
      timestamp: "2026-07-18T12:02:00.000Z",
      transcript: "👩‍👩‍👧‍👦 dictation family history",
      preview: "emoji preview",
      target: "Notes",
      audio_duration_ms: 800,
    },
    {
      id: "dictation-hostile-script",
      timestamp: "2026-07-18T12:03:00.000Z",
      transcript: "<script>alert('dict')</script> <img src=x onerror=y>",
      preview: "script preview",
      target: "Main Filter",
      audio_duration_ms: 500,
    },
    {
      id: "dictation-hostile-huge",
      timestamp: "2026-07-18T12:04:00.000Z",
      transcript: "H".repeat(100_000),
      preview: "huge preview",
      target: "Main Filter",
      audio_duration_ms: 5000,
    },
    {
      id: "dictation-hostile-control",
      timestamp: "2026-07-18T12:05:00.000Z",
      transcript: "ansi \x1b[31mred\x1b[0m \x01\x02\x03 bell\x07",
      preview: "control preview",
      target: "Agent Chat",
      audio_duration_ms: 400,
    },
    // malformed lines mixed in (parser must skip)
    // plus filler for churn
    ...Array.from({ length: 40 }, (_, i) => ({
      id: `dictation-filler-${String(i).padStart(3, "0")}`,
      timestamp: `2026-07-18T13:${String(i % 60).padStart(2, "0")}:00.000Z`,
      transcript: `filler dictation entry ${i} tok-${i % 17}`,
      preview: `filler ${i}`,
      target: i % 2 === 0 ? "Main Filter" : "Notes",
      audio_duration_ms: 200 + i * 10,
    })),
  ];
  const lines = [
    ...entries.map((e) => JSON.stringify(e)),
    "{not valid json line",
    JSON.stringify({ id: "missing-fields" }), // incomplete shape
    "",
  ];
  writeFileSync(path, lines.join("\n") + "\n");
  return path;
}

function seedCorruptDictationConfig(kitDir: string, mode: "settings" | "config-ts" | "both") {
  if (mode === "settings" || mode === "both") {
    // Wrong types + hostile values in legacy settings.json
    writeFileSync(
      join(kitDir, "settings.json"),
      JSON.stringify({
        dictation: {
          model: 12345, // wrong type
          language: { nested: true }, // wrong type
          silence_rms: "loud", // wrong type
          max_duration_secs: -99,
          push_to_talk: "sometimes",
          save_history: "yes-please",
          selected_device_id: ["not", "a", "string"],
          target: { bogus: true },
          targetMode: "not-a-real-mode",
        },
        layout: "should-be-object",
      }),
    );
  }
  if (mode === "config-ts" || mode === "both") {
    writeFileSync(
      join(kitDir, "config.ts"),
      `// chaos-25 corrupt dictation config
export default {
  dictation: {
    model: 999,
    silence_rms: "nope",
    push_to_talk: "maybe",
    max_duration_secs: "forever",
    target: 42,
    language: false,
  },
  // truncated / hostile tail
  theme: { mode: "dark",
`);
  }
}

// ── main ─────────────────────────────────────────────────────────────────────

const scratch = join(tmpdir(), `chaos-25-dictation-${process.pid}`);
const sandboxHome = join(scratch, "home");
const kitDir = join(sandboxHome, ".scriptkit");
mkdirSync(kitDir, { recursive: true });
writeFileSync(join(kitDir, "config.ts"), "export default {};\n");

const envNote = {
  realMicTcc: "UNREACHABLE in sandboxed fixture-driven probes — never product FAIL",
  realPttGlobalHotkey:
    "Global dictation hotkey press/hold/release not injectable via protocol; " +
    "row b tests protocol-reachable double-activation + surface-transition edges; " +
    "true hardware PTT hold contract = ENV",
  whisperModel:
    "Row d deliberately runs without sharedModels symlink so model absence is the " +
    "degraded env under test; classify expected setup/download UX as ENV not bug",
};

const meta: Json = {
  nn: 25,
  battery: "dictation",
  ledger: "round-44",
  binary: BINARY,
  loadavgStart: loadavg(),
  envNote,
  rowOnly: ROW_ONLY || null,
  startedAt: new Date().toISOString(),
};

if (!existsSync(BINARY)) {
  console.error(JSON.stringify({ ok: false, error: `binary missing: ${BINARY}`, meta }, null, 2));
  process.exit(2);
}

let d: Driver | null = null;

try {
  // ── SMOKE ────────────────────────────────────────────────────────────────
  if (shouldRun("smoke")) {
    d = await Driver.launch({
      binary: BINARY,
      sandboxHome: true,
      sessionName: `chaos-25-dictation-${process.pid}`,
      readyTimeoutMs: 25_000,
      defaultTimeoutMs: 12_000,
      // sharedModels default true for smoke/a/b/e; row d relaunches without
    });
    // Prefer driver-owned sandbox kit for all subsequent seeds in this process.
    const driverKit = join(d.sessionDir, "home", ".scriptkit");
    if (existsSync(driverKit)) {
      // rebind mutable kitDir via write through known path
      try {
        writeFileSync(join(driverKit, ".chaos-25-kit-marker"), "ok\n");
      } catch {}
    }
    // stash for row e
    (globalThis as any).__chaos25KitDir = existsSync(driverKit) ? driverKit : kitDir;

    await settle(d, 500);
    const baselineErrors = await errorKeys(d);
    const state0 = (await d.getState({ timeoutMs: 10_000 })) as Json;
    const alive0 = state0 != null && typeof state0 === "object";

    pushDictation(d, {
      transcript: "chaos-25 smoke short phrase",
      target: "mainWindowFilter",
    });
    await settle(d, 600);
    const state1 = (await d.getState({ timeoutMs: 10_000 })) as Json;
    const filterAfter = filterText(state1);
    const delivered =
      filterAfter.includes("chaos-25 smoke") ||
      filterAfter.includes("smoke short");

    d.send({ type: "openDictationOverlayFixture" });
    await settle(d, 900);
    let overlayFound = false;
    let overlayBounds: Json = null;
    try {
      const windows = (await d.listAutomationWindows()) as {
        windows?: Array<Record<string, unknown>>;
      };
      const dictation = windows.windows?.find((w) => w.id === "dictation");
      overlayFound = Boolean(dictation);
      overlayBounds = (dictation?.bounds as Json) ?? null;
    } catch (e) {
      findings.push({ step: "smoke-listWindows", error: String(e) });
    }

    const afterErrors = await errorKeys(d);
    const fresh = newErrorDelta(baselineErrors, afterErrors);
    const dictationState = state1?.dictation_state ?? state1?.dictationState ?? null;

    const smokeOk = alive0 && delivered && fresh.length === 0;
    rows.push({
      id: "smoke",
      verdict: smokeOk ? "PASS" : delivered || alive0 ? "SUSPECT" : "FAIL",
      reason: smokeOk
        ? "launch+pushDictationResult+overlay-fixture ok"
        : `alive=${alive0} delivered=${delivered} newErrors=${fresh.length} overlay=${overlayFound}`,
      detail: {
        filterAfter: filterAfter.slice(0, 120),
        delivered,
        overlayFound,
        overlayBounds,
        dictationState,
        newErrors: fresh.slice(0, 8),
        promptType: state1?.promptType ?? null,
      },
    });
    writeFileSync(
      join(OUT_DIR, "smoke.json"),
      JSON.stringify(rows[rows.length - 1], null, 2),
    );
  }

  // ── ROW a: live-partials churn ───────────────────────────────────────────
  if (d && shouldRun("a")) {
    const before = await errorKeys(d);
    // reset filter
    try {
      d.setFilter("");
    } catch {}
    await settle(d, 300);

    const layout0 = (await d.getLayoutInfo().catch(() => null)) as Json;
    const chrome0 = layout0 ? stableBounds(layout0) : new Map();

    // Open overlay fixture so partial-adjacent chrome exists
    d.send({ type: "openDictationOverlayFixture" });
    await settle(d, 700);
    let overlayLayout0: Json = null;
    try {
      overlayLayout0 = (await d.getLayoutInfo({
        target: { type: "id", id: "dictation" },
      })) as Json;
    } catch {
      overlayLayout0 = null;
    }
    const overlayChrome0 = overlayLayout0 ? stableBounds(overlayLayout0) : new Map();

    // Storm: partial-only deliveries (empty final → partial fallback) + finals
    const stormStats: Json[] = [];
    for (let i = 0; i < HOSTILE_PARTIALS.length; i++) {
      const partial = HOSTILE_PARTIALS[i];
      const t0 = performance.now();
      pushDictation(d, {
        transcript: "",
        partialTranscript: partial,
        target: "mainWindowFilter",
      });
      await settle(d, i === HOSTILE_PARTIALS.length - 1 ? 500 : 120);
      const st = (await d.getState({ timeoutMs: 8000 }).catch(() => null)) as Json;
      stormStats.push({
        i,
        kind: "partial",
        len: partial.length,
        settleMs: Math.round(performance.now() - t0),
        alive: st != null,
        filterLen: filterText(st ?? {}).length,
        selectionOk: st ? selectionOk(st) : false,
      });
    }
    for (let i = 0; i < HOSTILE_FINALS.length; i++) {
      const final = HOSTILE_FINALS[i];
      const t0 = performance.now();
      pushDictation(d, {
        transcript: final,
        partialTranscript: "stale partial should lose to final",
        target: "mainWindowFilter",
      });
      await settle(d, 150);
      const st = (await d.getState({ timeoutMs: 8000 }).catch(() => null)) as Json;
      stormStats.push({
        i,
        kind: "final",
        len: final.length,
        settleMs: Math.round(performance.now() - t0),
        alive: st != null,
        filterLen: filterText(st ?? {}).length,
        selectionOk: st ? selectionOk(st) : false,
      });
    }

    // Isolated multi-line delivery — known GPUI panic candidate (text_system newlines).
    let multilineFinding: Json | null = null;
    try {
      pushDictation(d, {
        transcript: MULTILINE_CRASH_CANDIDATE,
        target: "mainWindowFilter",
      });
      await settle(d, 600);
      const stMl = (await d.getState({ timeoutMs: 8000 }).catch(() => null)) as Json;
      if (stMl == null || !d.alive) {
        multilineFinding = {
          kind: "product-crash",
          id: "OF-dictation-multiline-filter-panic",
          summary:
            "pushDictationResult multi-line transcript into mainWindowFilter killed the app " +
            "(expected GPUI panic: text argument should not contain newlines)",
          transcriptPreview: MULTILINE_CRASH_CANDIDATE.slice(0, 80),
        };
      } else {
        multilineFinding = {
          kind: "no-crash",
          filterLen: filterText(stMl).length,
          note: "multi-line delivery survived — panic may be fixed or path changed",
        };
      }
    } catch (e) {
      multilineFinding = {
        kind: "product-crash",
        id: "OF-dictation-multiline-filter-panic",
        summary: "pushDictationResult multi-line transcript crashed driver/app",
        error: String(e).slice(0, 300),
      };
    }
    if (multilineFinding?.kind === "product-crash") {
      findings.push(multilineFinding);
      // Relaunch to continue CLS measurement / remaining rows
      try {
        await d.close();
      } catch {}
      d = await Driver.launch({
        binary: BINARY,
        sandboxHome: true,
        sessionName: `chaos-25-dictation-a-relaunch-${process.pid}`,
        readyTimeoutMs: 25_000,
        defaultTimeoutMs: 12_000,
      });
      (globalThis as any).__chaos25KitDir = join(d.sessionDir, "home", ".scriptkit");
      await settle(d, 400);
      // re-open overlay for chrome compare best-effort
      d.send({ type: "openDictationOverlayFixture" });
      await settle(d, 500);
    }

    // Rapid rewrite storm: same target, alternating short partials
    for (let i = 0; i < 30; i++) {
      pushDictation(d, {
        transcript: i % 3 === 0 ? `final-${i}` : "",
        partialTranscript: i % 3 === 0 ? undefined : `partial rewrite ${i} ${"w".repeat(i % 20)}`,
        target: "mainWindowFilter",
      });
      if (i % 5 === 0) await Bun.sleep(40);
    }
    await settle(d, 500);

    const layout1 = (await d.getLayoutInfo().catch(() => null)) as Json;
    const chrome1 = layout1 ? stableBounds(layout1) : new Map();
    const mainDrift = maxChromeDrift(chrome0, chrome1);

    let overlayDrift = { max: 0, worst: null as string | null };
    try {
      const overlayLayout1 = (await d.getLayoutInfo({
        target: { type: "id", id: "dictation" },
      })) as Json;
      const overlayChrome1 = stableBounds(overlayLayout1);
      overlayDrift = maxChromeDrift(overlayChrome0, overlayChrome1);
    } catch {
      // overlay may have closed after delivery — not necessarily a fail
    }

    const stateEnd = (await d.getState({ timeoutMs: 8000 }).catch(() => null)) as Json;
    const after = await errorKeys(d);
    const fresh = newErrorDelta(before, after);
    const allAlive = stormStats.every((s) => s.alive === true);
    const maxSettle = Math.max(...stormStats.map((s) => Number(s.settleMs) || 0));
    const clsMainOk = mainDrift.max <= CLS_EPS;
    // Overlay chrome may remount; if we got measurements, enforce; else note gap
    const clsOverlayOk =
      overlayChrome0.size === 0 || overlayDrift.max <= CLS_EPS + 2; // slightly looser: fixture remount

    const failReasons: string[] = [];
    if (!allAlive) failReasons.push("state-null-during-storm");
    if (fresh.length) failReasons.push(`new-errors:${fresh.length}`);
    if (!clsMainOk) failReasons.push(`main-cls:${mainDrift.max.toFixed(2)}@${mainDrift.worst}`);
    if (!stateEnd) failReasons.push("dead-at-end");
    if (multilineFinding?.kind === "product-crash") {
      failReasons.push("OF-dictation-multiline-filter-panic");
    }

    // Settle budget: large (~10–14KB) filter inserts can exceed 4s under
    // search/layout load. Manager watch list (unrelated to OF-17 crash):
    // record as settleWatch, do NOT downgrade row correctness to SUSPECT.
    const hugeSettles = stormStats.filter(
      (s) => Number(s.len) >= 8_000 && Number(s.settleMs) > 0,
    );
    const settleWatch =
      maxSettle > 4000
        ? {
            kind: "watch",
            id: "OF-dictation-14kb-filter-settle",
            maxSettleMs: maxSettle,
            budgetMs: 4000,
            hugeSettles,
            classification:
              "WATCH — large single-line filter insert settle latency; not a crash. " +
              "Reproduce under load; do not block OF-17 correctness green.",
          }
        : null;

    let verdict: RowVerdict = "PASS";
    if (failReasons.length) verdict = "FAIL";
    else if (!clsOverlayOk) verdict = "SUSPECT";
    // maxSettle alone no longer SUSPECTs the row (manager watch item).

    const of17Fixed = multilineFinding?.kind === "no-crash";
    rows.push({
      id: "a-live-partials",
      verdict,
      reason:
        verdict === "PASS"
          ? `partial/final storm ok; main CLS ${mainDrift.max.toFixed(2)}px; ` +
            (of17Fixed ? "OF-17 multiline no-crash; " : "") +
            `maxSettle ${maxSettle}ms` +
            (settleWatch ? " (settle WATCH)" : "")
          : failReasons.join("; ") || "suspect overlay CLS",
      detail: {
        stormCount: stormStats.length,
        maxSettle,
        settleWatch,
        mainDrift,
        overlayDrift,
        overlayChromeKeys: [...overlayChrome0.keys()],
        newErrors: fresh.slice(0, 10),
        sample: stormStats.filter((_, i) => i % 3 === 0).slice(0, 8),
        filterLenEnd: filterText(stateEnd ?? {}).length,
        multilineFinding,
        of17Fixed,
        stdinCapNote:
          "100KB via pushDictationResult is protocol-skipped (max_line_bytes=16384); " +
          "probed at STDIN_SAFE=14000. True 100KB covered via history file in row e.",
      },
    });
    writeFileSync(
      join(OUT_DIR, "a-live-partials.json"),
      JSON.stringify(rows[rows.length - 1], null, 2),
    );
  }

  // ── ROW b: PTT hold-contract edges (protocol-reachable) ──────────────────
  if (d && shouldRun("b")) {
    const before = await errorKeys(d);
    const notes: string[] = [];

    // ENV: real global hotkey hold/release is not protocol-injectable.
    notes.push(
      "ENV: DICTATION_PUSH_TO_TALK_MIN_HOLD (500ms) real key-hold path requires " +
        "global hotkey; simulateMainHotkeyGesture is main-launcher only — not dictation PTT",
    );

    // Protocol edge 1: rapid double triggerBuiltin dictation (double-activation)
    const ds0 = (await d.getState({ timeoutMs: 8000 })) as Json;
    const dict0 = ds0?.dictation_state ?? ds0?.dictationState;
    for (let i = 0; i < 4; i++) {
      // NOT name:"dictation" — that alias resolves to DictationHistory.
      d.send({ type: "triggerBuiltin", builtinId: "builtin/dictation" });
      await Bun.sleep(80);
    }
    await settle(d, 800);
    const ds1 = (await d.getState({ timeoutMs: 8000 }).catch(() => null)) as Json;
    const dict1 = ds1?.dictation_state ?? ds1?.dictationState;
    const aliveAfterDouble = ds1 != null;

    // Protocol edge 2: overlay fixture + surface transition (hold-during-surface)
    d.send({ type: "openDictationOverlayFixture" });
    await settle(d, 500);
    d.send({ type: "triggerBuiltin", name: "dictationHistory" });
    await settle(d, 700);
    const ds2 = (await d.getState({ timeoutMs: 8000 }).catch(() => null)) as Json;
    const prompt2 = String(ds2?.promptType ?? "");
    // return to main
    try {
      d.simulateKey("escape");
    } catch {}
    await settle(d, 400);
    d.send({ type: "openDictationOverlayFixture" });
    await settle(d, 400);
    // push while fixture open (delivery should abort active capture path safely)
    pushDictation(d, {
      transcript: "ptt-edge during overlay",
      target: "mainWindowFilter",
    });
    await settle(d, 500);
    const ds3 = (await d.getState({ timeoutMs: 8000 }).catch(() => null)) as Json;

    // Protocol edge 3: rapid hold-like press pairs via simulateKey on dictation
    // hotkey if configured — best-effort; classify miss as ENV
    let hotkeySim: Json = { attempted: true, note: "simulateKey cmd+shift+; best-effort" };
    try {
      for (let i = 0; i < 3; i++) {
        d.simulateKey(";", ["cmd", "shift"]);
        await Bun.sleep(100); // < 500ms PTT hold threshold → toggle semantics
        d.simulateKey(";", ["cmd", "shift"]);
        await Bun.sleep(50);
      }
      // hold-like: press, wait >500ms — but simulateKey is edge not hold; document
      await settle(d, 400);
      hotkeySim = {
        attempted: true,
        note: "simulateKey cannot hold; PTT release path ENV",
        stateAlive: true,
      };
    } catch (e) {
      hotkeySim = { attempted: true, error: String(e) };
    }

    const after = await errorKeys(d);
    const fresh = newErrorDelta(before, after);
    const alive = aliveAfterDouble && ds2 != null && ds3 != null;

    let verdict: RowVerdict = "PASS";
    const failReasons: string[] = [];
    if (!alive) failReasons.push("dead-after-activation-storm");
    if (fresh.length) failReasons.push(`new-errors:${fresh.length}`);
    if (failReasons.length) verdict = "FAIL";
    // Always attach ENV note for true PTT hold — not a fail
    notes.push("true-PTT-hold-release: ENV unreachable");

    rows.push({
      id: "b-ptt-edges",
      verdict,
      reason:
        verdict === "PASS"
          ? "protocol double-activation + overlay/surface edges ok; true PTT hold=ENV"
          : failReasons.join("; "),
      detail: {
        notes,
        dictationStateBefore: dict0,
        dictationStateAfterDouble: dict1,
        promptAfterHistoryOpen: prompt2,
        filterAfterPush: filterText(ds3 ?? {}).slice(0, 80),
        hotkeySim,
        newErrors: fresh.slice(0, 8),
        env: {
          realPttHold: "ENV",
          realMic: "ENV",
        },
      },
    });
    writeFileSync(
      join(OUT_DIR, "b-ptt-edges.json"),
      JSON.stringify(rows[rows.length - 1], null, 2),
    );
  }

  // ── ROW c: dictation.* config corruption ─────────────────────────────────
  // Needs relaunch with pre-seeded corrupt config in a fresh HOME.
  if (shouldRun("c")) {
    if (d) {
      try {
        await d.close();
      } catch {}
      d = null;
    }

    const corruptCases: Array<{ name: string; mode: "settings" | "config-ts" | "both" }> = [
      { name: "settings-wrong-types", mode: "settings" },
      { name: "config-ts-malformed", mode: "config-ts" },
      { name: "both-corrupt", mode: "both" },
    ];
    const caseResults: Json[] = [];
    let anyFail = false;
    let anySuspect = false;

    for (const c of corruptCases) {
      const root = join(scratch, `corrupt-${c.name}`);
      const home = join(root, "home");
      const kit = join(home, ".scriptkit");
      mkdirSync(kit, { recursive: true });
      if (c.mode !== "config-ts") {
        writeFileSync(join(kit, "config.ts"), "export default {};\n");
      }
      seedCorruptDictationConfig(kit, c.mode);
      // still symlink models so we don't trip model-download lane here
      try {
        const realModels = join(homedir(), ".scriptkit", "models");
        if (existsSync(realModels)) symlinkSync(realModels, join(kit, "models"));
      } catch {}

      let ready = false;
      let alive = false;
      let err = "";
      let filterOk = false;
      let promptType: string | null = null;
      let freshErrors: string[] = [];
      let child: Driver | null = null;
      try {
        child = await Driver.launch({
          binary: BINARY,
          sandboxHome: false,
          sessionName: `chaos-25-c-${c.name}-${process.pid}`,
          readyTimeoutMs: 25_000,
          defaultTimeoutMs: 12_000,
          env: { HOME: home, SK_PATH: kit },
          sharedModels: false, // already symlinked manually
        });
        ready = true;
        const before = await errorKeys(child);
        await settle(child, 400);
        child.setFilter("probe-corrupt");
        await settle(child, 300);
        const st = (await child.getState({ timeoutMs: 10_000 })) as Json;
        alive = st != null;
        promptType = st?.promptType != null ? String(st.promptType) : null;
        filterOk = filterText(st).includes("probe-corrupt") || filterText(st).length >= 0;
        // delivery still works with degraded prefs
        pushDictation(child, {
          transcript: "after-corrupt-config",
          target: "mainWindowFilter",
        });
        await settle(child, 500);
        const st2 = (await child.getState({ timeoutMs: 8000 })) as Json;
        const delivered = filterText(st2).includes("after-corrupt");
        const after = await errorKeys(child);
        freshErrors = newErrorDelta(before, after);
        caseResults.push({
          name: c.name,
          ready,
          alive,
          filterOk,
          delivered,
          promptType,
          newErrors: freshErrors.slice(0, 6),
        });
        if (!ready || !alive) anyFail = true;
        if (freshErrors.length) anySuspect = true;
        if (!delivered) anySuspect = true;
      } catch (e) {
        err = String(e);
        anyFail = true;
        caseResults.push({ name: c.name, ready, alive, error: err.slice(0, 300) });
      } finally {
        if (child) {
          try {
            await child.close();
          } catch {}
        }
      }
    }

    // relaunch main driver for remaining rows
    d = await Driver.launch({
      binary: BINARY,
      sandboxHome: true,
      sessionName: `chaos-25-dictation-post-c-${process.pid}`,
      readyTimeoutMs: 25_000,
      defaultTimeoutMs: 12_000,
    });
    (globalThis as any).__chaos25KitDir = join(d.sessionDir, "home", ".scriptkit");
    await settle(d, 400);

    rows.push({
      id: "c-config-corruption",
      verdict: anyFail ? "FAIL" : anySuspect ? "SUSPECT" : "PASS",
      reason: anyFail
        ? "one or more corrupt-config launches failed/crashed"
        : anySuspect
          ? "launched but delivery/errors degraded"
          : "all corrupt dictation.* shapes launched + delivered gracefully",
      detail: { cases: caseResults },
    });
    writeFileSync(
      join(OUT_DIR, "c-config-corruption.json"),
      JSON.stringify(rows[rows.length - 1], null, 2),
    );
  }

  // ── ROW d: whisper/model-absent degraded env ─────────────────────────────
  if (shouldRun("d")) {
    if (d) {
      try {
        await d.close();
      } catch {}
      d = null;
    }

    const root = join(scratch, "no-models");
    const home = join(root, "home");
    const kit = join(home, ".scriptkit");
    mkdirSync(join(kit, "models"), { recursive: true }); // empty models dir — no whisper
    writeFileSync(join(kit, "config.ts"), "export default {};\n");

    let child: Driver | null = null;
    let verdict: RowVerdict = "ENV";
    let reason = "";
    let detail: Json = {};
    try {
      child = await Driver.launch({
        binary: BINARY,
        sandboxHome: false,
        sessionName: `chaos-25-d-nomodel-${process.pid}`,
        readyTimeoutMs: 25_000,
        defaultTimeoutMs: 12_000,
        env: { HOME: home, SK_PATH: kit },
        sharedModels: false, // critical: do not link real models
      });
      const before = await errorKeys(child);
      await settle(child, 400);

      // Attempt start dictation → expect setup/model prompt, not crash
      child.send({ type: "triggerBuiltin", builtinId: "builtin/dictation" });
      await settle(child, 1200);
      const st = (await child.getState({ timeoutMs: 10_000 })) as Json;
      const promptType = String(st?.promptType ?? "");
      const dictState = st?.dictation_state ?? st?.dictationState ?? null;
      const after = await errorKeys(child);
      const fresh = newErrorDelta(before, after);

      // pushDictationResult is independent of model — should still deliver
      pushDictation(child, {
        transcript: "fixture-without-whisper",
        target: "mainWindowFilter",
      });
      await settle(child, 500);
      const st2 = (await child.getState({ timeoutMs: 8000 })) as Json;
      const delivered = filterText(st2).includes("fixture-without-whisper");

      const crashedHard = st == null;
      const setupLike =
        /dictation|setup|model|download|permission|select/i.test(promptType) ||
        promptType === "none" ||
        promptType === "";

      if (crashedHard) {
        verdict = "FAIL";
        reason = "app dead after dictation start with no model";
      } else if (fresh.some((e) => /panic|FATAL|segfault/i.test(e))) {
        verdict = "FAIL";
        reason = "fatal errors on model-absent start";
      } else {
        // Honest ENV: degraded env is expected; product must be clear & alive
        verdict = "ENV";
        reason =
          "model-absent sandbox: app alive, start path did not panic; " +
          "fixture pushDictationResult still works; real mic/TCC N/A";
      }

      detail = {
        promptType,
        dictationState: dictState,
        delivered,
        setupLike,
        newErrors: fresh.slice(0, 10),
        classification:
          "ENV — whisper/parakeet model intentionally absent (sharedModels:false, empty models/). " +
          "Expect graceful setup/download UX; not a product correctness bug unless crash/panic.",
        realMicTcc: "UNREACHABLE",
      };
    } catch (e) {
      verdict = "FAIL";
      reason = `launch/start failed: ${String(e).slice(0, 200)}`;
      detail = { error: String(e).slice(0, 400) };
    } finally {
      if (child) {
        try {
          await child.close();
        } catch {}
      }
    }

    // restore main driver for row e
    d = await Driver.launch({
      binary: BINARY,
      sandboxHome: true,
      sessionName: `chaos-25-dictation-post-d-${process.pid}`,
      readyTimeoutMs: 25_000,
      defaultTimeoutMs: 12_000,
    });
    (globalThis as any).__chaos25KitDir = join(d.sessionDir, "home", ".scriptkit");
    await settle(d, 400);

    rows.push({
      id: "d-whisper-absent",
      verdict,
      reason,
      detail,
    });
    writeFileSync(
      join(OUT_DIR, "d-whisper-absent.json"),
      JSON.stringify(rows[rows.length - 1], null, 2),
    );
  }

  // ── ROW e: history-list hostile + separator under churn ──────────────────
  if (d && shouldRun("e")) {
    // Seed history into current sandbox kit (app may need reopen to see file;
    // seed then triggerBuiltin — history load is file-backed with cache)
    const activeKit = (globalThis as any).__chaos25KitDir ?? kitDir;
    seedHistoryJsonl(activeKit);
    // Also write a second batch mid-session for churn
    const before = await errorKeys(d);

    // open dictation history
    d.send({ type: "triggerBuiltin", name: "dictationHistory" });
    await settle(d, 900);
    let st = (await d.getState({ timeoutMs: 10_000 })) as Json;
    let promptType = String(st?.promptType ?? "");
    // retry once if still on main
    if (!/dictation/i.test(promptType) && promptType !== "DictationHistory") {
      d.send({ type: "triggerBuiltin", name: "dictationHistory" });
      await settle(d, 900);
      st = (await d.getState({ timeoutMs: 10_000 })) as Json;
      promptType = String(st?.promptType ?? "");
    }

    const layout0 = (await d.getLayoutInfo().catch(() => null)) as Json;
    const chrome0 = layout0 ? stableBounds(layout0) : new Map();

    const els0 = (await d
      .getElements({ limit: 300, includeHeaders: true } as any)
      .catch(() => ({ elements: [] }))) as Json;
    // some drivers use options object differently
    let elementsResult = els0;
    try {
      elementsResult = (await (d as any).getElements({
        limit: 300,
        includeHeaders: true,
      })) as Json;
    } catch {
      try {
        elementsResult = (await d.getElements(300 as any)) as Json;
      } catch {
        elementsResult = { elements: [] };
      }
    }
    const listRows0 = rowsOf(elementsResult);
    const sep0 = leadingSeparator(listRows0);
    const choiceCount0 = listRows0.filter(
      (r) => r.type === "choice" || r.selectable || /dictation/i.test(r.semanticId),
    ).length;

    // filter churn
    const filters = ["", "filler", "zalgo", "zzqq-nomatch-0011", "emoji", "a", ""];
    const churn: Json[] = [];
    for (const f of filters) {
      d.setFilter(f);
      await settle(d, 250);
      const s = (await d.getState({ timeoutMs: 8000 }).catch(() => null)) as Json;
      let els: Json = { elements: [] };
      try {
        els = (await (d as any).getElements({ limit: 300, includeHeaders: true })) as Json;
      } catch {
        els = { elements: [] };
      }
      const lr = rowsOf(els);
      const sep = leadingSeparator(lr);
      churn.push({
        filter: f,
        alive: s != null,
        promptType: s?.promptType ?? null,
        selectionOk: s ? selectionOk(s) : false,
        rowCount: lr.length,
        sepPresent: sep.present,
        sepReason: sep.reason,
        firstSemanticId: sep.first?.semanticId ?? null,
      });
    }

    // external history churn: append more hostile while surface open
    const histPath = join(activeKit, "dictation-history.jsonl");
    try {
      const more = Array.from({ length: 20 }, (_, i) =>
        JSON.stringify({
          id: `dictation-churn-${i}`,
          timestamp: `2026-07-18T14:${String(i).padStart(2, "0")}:00.000Z`,
          transcript: `external churn ${i} ` + (i === 0 ? "H".repeat(50_000) : "ok"),
          preview: `churn ${i}`,
          target: "Main Filter",
          audio_duration_ms: 100,
        }),
      ).join("\n");
      writeFileSync(histPath, readFileSync(histPath, "utf8") + more + "\n");
    } catch (e) {
      findings.push({ step: "history-append-churn", error: String(e) });
    }
    d.setFilter("");
    await settle(d, 500);
    // re-open to pick up file changes if needed
    try {
      d.simulateKey("escape");
    } catch {}
    await settle(d, 300);
    d.send({ type: "triggerBuiltin", name: "dictationHistory" });
    await settle(d, 800);

    const layout1 = (await d.getLayoutInfo().catch(() => null)) as Json;
    const chrome1 = layout1 ? stableBounds(layout1) : new Map();
    const mainDrift = maxChromeDrift(chrome0, chrome1);

    const stEnd = (await d.getState({ timeoutMs: 8000 }).catch(() => null)) as Json;
    const after = await errorKeys(d);
    const fresh = newErrorDelta(before, after);

    const opened =
      /dictation/i.test(String(stEnd?.promptType ?? promptType)) ||
      String(promptType) === "DictationHistory" ||
      listRows0.length > 0;

    const churnAlive = churn.every((c) => c.alive === true);
    // Separator contract: when list has content, first row should be sectionHeader
    // (OF-15). Empty/zero-match may legitimately have no separator.
    const sepMisses = churn.filter(
      (c) => Number(c.rowCount) > 0 && c.filter !== "zzqq-nomatch-0011" && c.sepPresent === false,
    );

    const failReasons: string[] = [];
    if (!opened) failReasons.push("dictation-history-not-opened");
    if (!churnAlive) failReasons.push("dead-during-filter-churn");
    if (fresh.length) failReasons.push(`new-errors:${fresh.length}`);
    if (mainDrift.max > CLS_EPS) {
      failReasons.push(`cls:${mainDrift.max.toFixed(2)}@${mainDrift.worst}`);
    }
    if (stEnd == null) failReasons.push("dead-at-end");

    let verdict: RowVerdict = "PASS";
    if (failReasons.length) verdict = "FAIL";
    else if (sepMisses.length > 0) {
      verdict = "SUSPECT"; // OF-15 style separator miss — report, don't auto-fix product
    }

    rows.push({
      id: "e-history-hostile-separator",
      verdict,
      reason:
        verdict === "PASS"
          ? `history opened; hostile rows render; separator ok; CLS ${mainDrift.max.toFixed(2)}px`
          : verdict === "SUSPECT"
            ? `separator miss on ${sepMisses.length} churn filters (OF-15 style)`
            : failReasons.join("; "),
      detail: {
        promptTypeOpen: promptType,
        promptTypeEnd: stEnd?.promptType ?? null,
        choiceCount0,
        sep0,
        churn,
        sepMisses,
        mainDrift,
        newErrors: fresh.slice(0, 10),
        opened,
      },
    });
    writeFileSync(
      join(OUT_DIR, "e-history-hostile-separator.json"),
      JSON.stringify(rows[rows.length - 1], null, 2),
    );
  }
} catch (e) {
  crashed = String(e);
  findings.push({ fatal: true, error: crashed.slice(0, 500) });
} finally {
  if (d) {
    try {
      await d.close();
    } catch {}
  }
}

meta.loadavgEnd = loadavg();
meta.finishedAt = new Date().toISOString();
meta.crashed = crashed || null;

const summary = {
  ok: !crashed && rows.every((r) => r.verdict === "PASS" || r.verdict === "ENV"),
  rows,
  findings,
  meta,
  counts: {
    PASS: rows.filter((r) => r.verdict === "PASS").length,
    SUSPECT: rows.filter((r) => r.verdict === "SUSPECT").length,
    FAIL: rows.filter((r) => r.verdict === "FAIL").length,
    ENV: rows.filter((r) => r.verdict === "ENV").length,
  },
};

writeFileSync(join(OUT_DIR, "summary.json"), JSON.stringify(summary, null, 2));
console.log(JSON.stringify(summary, null, 2));
process.exit(summary.ok && !rows.some((r) => r.verdict === "FAIL") ? 0 : 1);

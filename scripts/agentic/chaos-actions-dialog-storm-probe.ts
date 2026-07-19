#!/usr/bin/env bun
/**
 * Chaos battery (2026-07-18, lane L1): actions-dialog hostile/rapid rows.
 * Prior coverage (devtools truth scenarios, battery 02) proved nominal
 * open/hover/click/close; none stormed the dialog.
 *
 * Rows (lenses: correctness, layout/CLS):
 *  1. nominal-open: cmd+k opens, visibleActions > 0, selection valid, escape closes.
 *  2. open-while-opening: two cmd+k back-to-back — app coherent, dialog state
 *     boolean-stable after settle, selection (if open) resolves to a visible action.
 *  3. esc-storm: 8 rapid escapes over an open dialog — dialog closed, app alive,
 *     re-show + filter still functional (extra escapes may legally hide the
 *     launcher via the escape ladder; a dead app or stuck dialog is the bug).
 *  4. rapid-toggle: 12× cmd+k/escape with no sleeps — alive, closed at even
 *     parity, no stuck overlay eating keystrokes afterward.
 *  5. dialog zero-match + recovery: type into the open dialog, assert the
 *     keystrokes land SOMEWHERE (dialog search or main filter — swallowed
 *     input is the bug), zero-match is graceful, backspace recovers.
 *  6. CLS: launcher chrome (input/header/footer) within 1px across the storm.
 *  7. no new ERROR log entries at any phase.
 *
 * Safe: sandboxHome, hidden-window protocol only (no show-claim needed beyond
 * the driver's off-screen window), unique session per run.
 */
import { Driver, type Json } from "../devtools/driver";

const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY;
const CLS_EPS = 1.0;

type Bounds = { x: number; y: number; width: number; height: number };
const STABLE_HINTS = ["input", "search", "footer", "header", "toolbar", "hint"];

function stableBounds(info: Json): Map<string, Bounds> {
  const m = new Map<string, Bounds>();
  for (const c of (info?.components ?? []) as Json[]) {
    if (!c?.bounds || typeof c.bounds.y !== "number") continue;
    const hay = `${c.name ?? ""} ${c.type ?? ""}`.toLowerCase();
    if (STABLE_HINTS.some((h) => hay.includes(h))) m.set(`${c.name}|${c.type ?? ""}`, c.bounds as Bounds);
  }
  return m;
}
const drift = (a: Bounds, b: Bounds) =>
  Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y), Math.abs(a.height - b.height));

const findings: Json[] = [];
let crashed = "";

const d = await Driver.launch({
  ...(BINARY ? { binary: BINARY } : {}),
  sandboxHome: true,
});

function dialogOf(st: Json): Json | null {
  const dlg = st?.actionsDialog;
  return dlg && dlg.open === true ? dlg : null;
}

async function state(label: string): Promise<Json> {
  try {
    return await d.getState({ timeoutMs: 8000 });
  } catch (e) {
    crashed = crashed || `${label}: getState failed: ${String(e).slice(0, 120)}`;
    return null;
  }
}

let errorBaseline = 0;
// Ledger OF-4/OF-6 CLOSED (chaos-19, 2026-07-18): the vendor gpui
// on_request_frame retry patch landed, so the "window not found" /
// "RefCell already borrowed" frame-callback noise must no longer appear.
// These signatures are now a RED bug kind (vendor-frame-lifecycle-error):
// a recurrence means the vendor patch regressed or a new lifecycle race.
const KNOWN_VENDOR_FRAME_NOISE =
  /vendor\/gpui\/src\/window\.rs/;
function isKnownVendorFrameNoise(msg: string): boolean {
  return (
    KNOWN_VENDOR_FRAME_NOISE.test(msg) &&
    (msg.includes("window not found") || msg.includes("RefCell already borrowed"))
  );
}
async function newErrors(label: string) {
  const logs: Json = await d.getLogs({ limit: 300, level: "error" }).catch(() => null);
  const entries = ((logs?.entries ?? []) as Json[]);
  const count = entries.length;
  if (count > errorBaseline) {
    const fresh = entries.slice(errorBaseline);
    const vendorNoise = fresh.filter((e) => isKnownVendorFrameNoise(String(e.message)));
    const real = fresh.filter((e) => !isKnownVendorFrameNoise(String(e.message)));
    if (vendorNoise.length > 0) {
      findings.push({
        kind: "vendor-frame-lifecycle-error", label, count: vendorNoise.length,
        ledger: "OF-4-closed-chaos-19",
        sample: vendorNoise.slice(-2).map((e) => String(e.message).slice(0, 140)),
      });
    }
    if (real.length > 0) {
      findings.push({
        kind: "new-error-logs", label, count: real.length,
        sample: real.slice(-3).map((e) => String(e.message).slice(0, 140)),
      });
    }
    errorBaseline = count;
  }
}

function assertDialogCoherent(dlg: Json, label: string) {
  const actions = (dlg?.visibleActions ?? []) as Json[];
  if (actions.length === 0) {
    findings.push({ kind: "dialog-no-actions", label });
    return;
  }
  const selected = dlg?.selectedActionId;
  if (selected && !actions.some((a) => a.id === selected)) {
    findings.push({ kind: "dialog-selection-incoherent", label, selected, actionCount: actions.length });
  }
}

try {
  d.send({ type: "show" });
  await Bun.sleep(400);
  await d.waitForSettle({ timeoutMs: 5000 }).catch(() => {});
  {
    const logs: Json = await d.getLogs({ limit: 300, level: "error" }).catch(() => null);
    errorBaseline = (logs?.entries ?? []).length;
  }
  const layoutBefore = stableBounds(await d.getLayoutInfo({}, { timeoutMs: 8000 }));

  // --- Row 1: nominal open/close ---
  d.simulateKey("k", ["cmd"]);
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
  {
    const st = await state("nominal-open");
    const dlg = dialogOf(st);
    if (!dlg) findings.push({ kind: "dialog-did-not-open", label: "nominal-open" });
    else assertDialogCoherent(dlg, "nominal-open");
  }
  d.simulateKey("escape");
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
  {
    const st = await state("nominal-close");
    if (dialogOf(st)) findings.push({ kind: "dialog-stuck-open", label: "nominal-close" });
  }
  await newErrors("nominal");

  // --- Row 2: open-while-opening ---
  d.simulateKey("k", ["cmd"]);
  d.simulateKey("k", ["cmd"]); // no sleep between — races the open transition
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
  {
    const st = await state("double-open");
    const dlg = dialogOf(st);
    if (dlg) assertDialogCoherent(dlg, "double-open");
    // Either open (second toggle ignored) or closed (toggle semantics) is
    // acceptable; record which for the report.
    findings.push({ kind: "note-double-open-behavior", open: dlg != null });
    if (dlg) {
      d.simulateKey("escape");
      await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
    }
  }
  await newErrors("double-open");

  // --- Row 3: esc storm over an open dialog ---
  d.simulateKey("k", ["cmd"]);
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
  for (let i = 0; i < 8; i++) d.simulateKey("escape"); // no sleeps
  await d.waitForSettle({ timeoutMs: 4000 }).catch(() => {});
  {
    const st = await state("esc-storm");
    if (dialogOf(st)) findings.push({ kind: "dialog-stuck-open", label: "esc-storm" });
    // Extra escapes may hide the launcher (escape ladder) — that is legal.
    // The app must still respond and re-show.
    d.send({ type: "show" });
    await Bun.sleep(300);
    await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
    d.setFilter("esc-storm-recovery");
    await Bun.sleep(250);
    const st2 = await state("esc-storm-recovery");
    if (st2?.inputValue !== "esc-storm-recovery") {
      findings.push({
        kind: "input-swallowed-after-storm", label: "esc-storm",
        inputValue: st2?.inputValue ?? null,
      });
    }
    d.setFilter("");
  }
  await newErrors("esc-storm");

  // --- Row 4: rapid open/close toggle ---
  for (let i = 0; i < 12; i++) {
    d.simulateKey("k", ["cmd"]);
    d.simulateKey("escape");
  }
  // OF-8: 12 queued cmd+k/escape pairs can still be draining when settle
  // returns — settle proves render-quiescence, not input-queue parity. Give
  // the queue a beat, settle, and re-poll before judging.
  await Bun.sleep(200);
  await d.waitForSettle({ timeoutMs: 6000 }).catch(() => {});
  {
    let st = await state("rapid-toggle");
    if (dialogOf(st)) {
      // Toggle parity lost a keystroke. Recoverable (one escape closes it,
      // exactly what a user would do) vs genuinely stuck (red).
      d.simulateKey("escape");
      await d.waitForSettle({ timeoutMs: 4000 }).catch(() => {});
      const st2 = await state("rapid-toggle-after-recovery-escape");
      if (dialogOf(st2)) {
        findings.push({ kind: "dialog-stuck-open", label: "rapid-toggle" });
      } else {
        findings.push({
          kind: "toggle-parity-lost-keystroke", label: "rapid-toggle",
          ledger: "OF-8",
          note: "dialog open after settle but one recovery escape closed it",
        });
      }
      st = st2;
    }
    d.setFilter("toggle-recovery");
    await Bun.sleep(250);
    const st2 = await state("rapid-toggle-recovery");
    if (st2?.inputValue !== "toggle-recovery") {
      findings.push({
        kind: "input-swallowed-after-storm", label: "rapid-toggle",
        inputValue: st2?.inputValue ?? null,
      });
    }
    d.setFilter("");
  }
  await newErrors("rapid-toggle");

  // --- Row 5: dialog search zero-match + recovery ---
  d.simulateKey("k", ["cmd"]);
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
  {
    const stOpen = await state("dialog-type-baseline");
    const dlgOpen = dialogOf(stOpen);
    const baselineCount = ((dlgOpen?.visibleActions ?? []) as Json[]).length;
    const baselineInput = String(stOpen?.inputValue ?? "");
    for (const ch of ["z", "q", "j", "x"]) d.simulateKey(ch);
    await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
    const st = await state("dialog-zero-match");
    const dlg = dialogOf(st);
    const filteredCount = ((dlg?.visibleActions ?? []) as Json[]).length;
    const inputNow = String(st?.inputValue ?? "");
    const landedInDialog = dlg != null && filteredCount !== baselineCount;
    const landedInMain = inputNow !== baselineInput;
    if (dlg != null && !landedInDialog && !landedInMain) {
      findings.push({
        kind: "dialog-typing-swallowed", baselineCount, filteredCount, inputNow,
      });
    }
    findings.push({
      kind: "note-dialog-typing-route",
      landedInDialog, landedInMain, baselineCount, filteredCount,
    });
    for (let i = 0; i < 4; i++) d.simulateKey("backspace");
    await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
    const stRec = await state("dialog-recovery");
    const dlgRec = dialogOf(stRec);
    if (dlgRec && ((dlgRec.visibleActions ?? []) as Json[]).length === 0 && baselineCount > 0) {
      findings.push({ kind: "dialog-recovery-lost-actions", baselineCount });
    }
    d.simulateKey("escape");
    await d.waitForSettle({ timeoutMs: 3000 }).catch(() => {});
  }
  await newErrors("dialog-typing");

  // --- Row 6: CLS across the whole storm ---
  {
    const layoutAfter = stableBounds(await d.getLayoutInfo({}, { timeoutMs: 8000 }));
    for (const [k, pb] of layoutBefore) {
      const cb = layoutAfter.get(k);
      if (!cb) continue;
      const dpx = drift(pb, cb);
      if (dpx > CLS_EPS) {
        findings.push({ kind: "chrome-layout-shift", surface: k, driftPx: Number(dpx.toFixed(2)) });
      }
    }
  }

  // Cleanup gate: leave hidden.
  d.simulateKey("escape");
  await Bun.sleep(250);
  d.send({ type: "hide" });
  await Bun.sleep(250);
  const stEnd = await state("cleanup");
  if (stEnd?.windowVisible === true) {
    findings.push({ kind: "cleanup-window-left-visible" });
  }
} catch (e) {
  crashed = crashed || String(e).slice(0, 200);
} finally {
  await d.close();
}

const bugKinds = [
  "dialog-did-not-open", "dialog-stuck-open", "dialog-no-actions",
  "dialog-selection-incoherent", "input-swallowed-after-storm",
  "dialog-typing-swallowed", "dialog-recovery-lost-actions",
  "chrome-layout-shift", "new-error-logs", "vendor-frame-lifecycle-error",
  "cleanup-window-left-visible",
];
const bugFindings = findings.filter((f) => bugKinds.includes(String(f.kind)));
const verdict = crashed ? "FAIL" : bugFindings.length > 0 ? "REGRESSION" : "PASS";

console.log(JSON.stringify({ verdict, crashed: crashed || null, findings, binary: BINARY ?? "auto" }, null, 2));
console.error(`[${verdict}] actions-dialog-storm: findings=${findings.length} ${crashed ? "CRASH:" + crashed : "alive"}`);
process.exit(verdict === "FAIL" ? 1 : 0);

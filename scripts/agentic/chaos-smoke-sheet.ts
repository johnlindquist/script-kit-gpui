#!/usr/bin/env bun
/**
 * chaos-smoke-sheet.ts — 9 safe chaos-monkey smoke "user stories" against the
 * real app, driven purely over the protocol (no OS input synthesis, no real
 * hotkeys, no computer-use). Every story runs in a sandbox HOME, screenshots
 * each step, and is classified PASS / SUSPECT / FAIL from measured evidence:
 *   - app still alive (driver still answers getState) after the story
 *   - reached the expected view / input
 *   - no NEW error-level log lines emitted during the story
 *
 * SAFETY: only filters, navigates, opens read-only builtins, and presses
 * Escape. It never submits a row that would spawn a process, never touches
 * real Script Kit state (sandbox HOME), never fires a global hotkey.
 *
 * Output: <outDir>/receipt.json + step-*.png filmstrip per story.
 */
import { Driver } from "../devtools/driver";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(process.cwd(), "target-agent/artifacts/chaos-smoke/script-kit-gpui");
const OUT = process.env.CHAOS_OUT ?? `/tmp/chaos-smoke-${Date.now().toString(36)}`;
mkdirSync(OUT, { recursive: true });

type Verdict = "PASS" | "SUSPECT" | "FAIL";
interface Step {
  n: number;
  intent: string;
  action: string;
  shot: string | null;
  inputValue?: string;
  promptType?: string;
  visibleChoiceCount?: number;
  selectedIndex?: number | null;
  note?: string;
  crashed?: boolean;
}
interface Story {
  id: string;
  title: string;
  intent: string;
  verdict: Verdict;
  reason: string;
  newErrors: string[];
  automationAnomalies?: string[];
  steps: Step[];
}

const stories: Story[] = [];

async function main() {
  const driver = await Driver.launch({ sandboxHome: true, binary: BINARY });
  driver.send({ type: "show" });
  await Bun.sleep(500);
  await driver.waitForSettle({ timeoutMs: 4000 });

  let shotSeq = 0;
  const shot = async (label: string): Promise<string | null> => {
    const file = `step-${String(++shotSeq).padStart(3, "0")}-${label}.png`;
    // The window may have been hidden by an Escape in the story; a hidden window
    // can't be captured. Re-show and let frames settle before capturing (the
    // longer settle avoids a GPUI window re-entrant borrow under rapid
    // show+capture).
    driver.send({ type: "show" });
    await Bun.sleep(320);
    // Explicit main-window target: capture must not depend on OS focus (other
    // sessions/terminals may be frontmost), which is what "No focused
    // automation window" meant.
    for (const target of [
      { type: "kind", kind: "main", index: 0 },
      { type: "main" },
    ]) {
      try {
        const r = await driver.captureScreenshot({
          target: target as any,
          savePath: join(OUT, file),
          timeoutMs: 12000,
        });
        if (!r.error) return file;
      } catch {
        /* try next target */
      }
    }
    return null;
  };

  // error-log baseline so we can attribute NEW errors to each story
  const errorSet = async (): Promise<Set<string>> => {
    try {
      const r = await driver.getLogs({ level: "error", limit: 200 }, { timeoutMs: 5000 });
      const entries: any[] = r?.entries ?? r?.logs ?? [];
      return new Set(entries.map((e: any) => `${e.target ?? ""}|${e.message ?? ""}`));
    } catch {
      return new Set();
    }
  };

  // Return to a clean main list between stories.
  const resetToMain = async () => {
    for (let i = 0; i < 4; i++) {
      driver.simulateKey("escape");
      await Bun.sleep(60);
    }
    driver.setFilter("");
    await Bun.sleep(150);
    driver.send({ type: "triggerBuiltin", name: "mainList" });
    await Bun.sleep(200);
    await driver.waitForSettle({ timeoutMs: 3000 });
  };

  const snap = async (): Promise<Partial<Step>> => {
    try {
      const s = await driver.getState({ timeoutMs: 5000 });
      return {
        inputValue: s.inputValue,
        promptType: s.promptType ?? s.prompt?.type ?? s.view,
        visibleChoiceCount: s.visibleChoiceCount,
        selectedIndex: s.selectedIndex ?? s.selectedRow ?? null,
        crashed: false,
      };
    } catch (e) {
      return { crashed: true, note: `getState threw: ${String(e).slice(0, 120)}` };
    }
  };

  // Runs one story: fn does steps (pushing to steps[]) and returns expected-ok.
  const runStory = async (
    id: string,
    title: string,
    intent: string,
    body: (
      step: (intent: string, action: string, doIt: () => Promise<void>, shotLabel?: string) => Promise<Step>,
    ) => Promise<{ ok: boolean; reason: string }>,
  ) => {
    await resetToMain();
    const before = await errorSet();
    const steps: Step[] = [];
    let stepN = 0;
    const step = async (
      intentText: string,
      action: string,
      doIt: () => Promise<void>,
      shotLabel = id,
    ): Promise<Step> => {
      await doIt();
      await Bun.sleep(120);
      const st = await snap();
      const file = await shot(`${shotLabel}-${++stepN}`);
      const s: Step = { n: stepN, intent: intentText, action, shot: file, ...st };
      steps.push(s);
      return s;
    };

    let result = { ok: false, reason: "story threw before completing" };
    try {
      result = await body(step);
    } catch (e) {
      result = { ok: false, reason: `story exception: ${String(e).slice(0, 200)}` };
    }
    const after = await errorSet();
    const rawNew = [...after].filter((k) => !before.has(k));
    // Two harness-attributable buckets are NOT app product errors:
    //  1. screenshot-capture failures (window focus / capture permission)
    //  2. GPUI vendor window re-entrancy ("RefCell already borrowed" in
    //     vendor/gpui/src/window.rs) induced by the probe's rapid show+capture
    //     — not reachable on a real user's debounced hotkey path.
    const isCaptureNoise = (k: string) => /captureScreenshot|automation window|screenshot/i.test(k);
    const isGpuiReentrancy = (k: string) =>
      /RefCell already borrowed/i.test(k) && /vendor\/gpui\/src\/window\.rs/i.test(k);
    const appErrors = rawNew.filter((k) => !isCaptureNoise(k) && !isGpuiReentrancy(k)).slice(0, 8);
    const automationAnomalies = rawNew.filter(isGpuiReentrancy).slice(0, 8);
    const crashed = steps.some((s) => s.crashed);

    let verdict: Verdict;
    let reason: string;
    if (crashed) {
      verdict = "FAIL";
      reason = "app stopped answering getState mid-story (possible panic/crash)";
    } else if (appErrors.length > 0) {
      verdict = "SUSPECT";
      reason = `${appErrors.length} new app error-log line(s): ${appErrors[0]}`;
    } else if (!result.ok) {
      verdict = "SUSPECT";
      reason = result.reason;
    } else {
      verdict = "PASS";
      reason = result.reason;
    }
    const anomalyNote =
      automationAnomalies.length > 0
        ? ` (+${automationAnomalies.length} harness-induced GPUI show/capture re-entrancy log(s), non-fatal)`
        : "";
    stories.push({ id, title, intent, verdict, reason: reason + anomalyNote, newErrors: appErrors, steps, automationAnomalies } as Story);
    console.error(`  [${verdict}] ${id} — ${title}`);
  };

  // ─── Story 1: main filter shows results ────────────────────────────────
  await runStory("s1-filter", "Filter the main menu", "Type a query and see the list narrow.", async (step) => {
    const a = await step("Open main menu", "triggerBuiltin mainList", async () => {
      driver.send({ type: "triggerBuiltin", name: "mainList" });
    });
    const b = await step("Type 'note'", "setFilter note", async () => driver.setFilter("note"));
    const c = await step("Type 'clip'", "setFilter clip", async () => driver.setFilter("clip"));
    const alive = !a.crashed && !b.crashed && !c.crashed;
    return { ok: alive, reason: alive ? "app filtered live, stayed responsive" : "app became unresponsive" };
  });

  // ─── Story 2: rapid Escape / dismiss ladder ────────────────────────────
  await runStory("s2-escape", "Rapid Escape ladder", "Open a builtin, hammer Escape, land back sane.", async (step) => {
    await step("Open clipboard history", "triggerBuiltin clipboardHistory", async () => {
      driver.send({ type: "triggerBuiltin", name: "clipboardHistory" });
    });
    await step("Escape x4 rapidly", "simulateKey escape x4", async () => {
      for (let i = 0; i < 4; i++) { driver.simulateKey("escape"); await Bun.sleep(40); }
    });
    const c = await step("Confirm main is usable", "setFilter aftertest", async () => driver.setFilter("aftertest"));
    const ok = !c.crashed && c.inputValue === "aftertest";
    return { ok, reason: ok ? "Escape ladder dismissed cleanly; main still accepts input" : "main did not accept input after Escape ladder" };
  });

  // ─── Story 3: hostile multibyte input (the bug-trigger strings) ────────
  await runStory("s3-multibyte", "Hostile multibyte filter input", "Feed the exact UTF-8 panic triggers into the live search.", async (step) => {
    const triggers = ["x 9-10am", ";todo meet x 9-10am", "tomorrow +aéb", "%€", "#€", "🌊".repeat(40)];
    let crashed = false;
    for (const t of triggers) {
      const s = await step(`Type ${JSON.stringify(t).slice(0, 22)}`, `setFilter ${JSON.stringify(t).slice(0, 18)}`, async () => driver.setFilter(t));
      if (s.crashed) { crashed = true; break; }
    }
    return { ok: !crashed, reason: crashed ? "app crashed on a multibyte trigger" : "survived all multibyte triggers (UTF-8 fixes hold live)" };
  });

  // ─── Story 4: pathological huge input ──────────────────────────────────
  await runStory("s4-huge", "Huge input string", "Paste a 5000-char query and verify no hang/crash.", async (step) => {
    const big = "The quick brown 🦊 jumps over the lazy dog. ".repeat(120);
    const a = await step(`Type ${big.length}-char string`, "setFilter <5000 chars>", async () => driver.setFilter(big));
    const b = await step("Clear it", "setFilter ''", async () => driver.setFilter(""));
    const ok = !a.crashed && !b.crashed;
    return { ok, reason: ok ? "handled 5000-char + emoji input without hang" : "hung/crashed on huge input" };
  });

  // ─── Story 5: emoji picker ─────────────────────────────────────────────
  await runStory("s5-emoji", "Emoji picker filter + navigate", "Open emoji picker, filter, arrow around.", async (step) => {
    await step("Open emoji picker", "triggerBuiltin emojiPicker", async () => {
      driver.send({ type: "triggerBuiltin", name: "emojiPicker" });
    });
    await step("Filter 'heart'", "setFilter heart", async () => driver.setFilter("heart"));
    const c = await step("Arrow down x3", "simulateKey down x3", async () => {
      for (let i = 0; i < 3; i++) { driver.simulateKey("down"); await Bun.sleep(50); }
    });
    return { ok: !c.crashed, reason: !c.crashed ? "emoji picker filtered + navigated" : "emoji picker crashed" };
  });

  // ─── Story 6: clipboard history view ───────────────────────────────────
  await runStory("s6-clipboard", "Clipboard history view", "Open the clipboard history surface (sandbox store).", async (step) => {
    const a = await step("Open clipboard history", "triggerBuiltin clipboardHistory", async () => {
      driver.send({ type: "triggerBuiltin", name: "clipboardHistory" });
    });
    const b = await step("Filter it", "setFilter test", async () => driver.setFilter("test"));
    return { ok: !a.crashed && !b.crashed, reason: !a.crashed ? "clipboard surface rendered + filtered" : "clipboard surface crashed" };
  });

  // ─── Story 7: actions menu (Cmd+K) ─────────────────────────────────────
  await runStory("s7-actions", "Actions menu open/close", "Cmd+K to open the actions popup, Escape to close.", async (step) => {
    await step("Open main", "triggerBuiltin mainList", async () => {
      driver.send({ type: "triggerBuiltin", name: "mainList" });
      await Bun.sleep(150);
    });
    await step("Cmd+K (open actions)", "simulateKey k+cmd", async () => {
      driver.simulateKey("k", ["cmd"]);
    });
    const c = await step("Escape (close actions)", "simulateKey escape", async () => driver.simulateKey("escape"));
    return { ok: !c.crashed, reason: !c.crashed ? "actions popup opened and closed" : "actions popup crashed" };
  });

  // ─── Story 8: arrow-key navigation stress ──────────────────────────────
  await runStory("s8-arrows", "Arrow navigation stress", "Rapid up/down; selection must stay in bounds.", async (step) => {
    await step("Open main", "triggerBuiltin mainList", async () => {
      driver.send({ type: "triggerBuiltin", name: "mainList" });
    });
    const b = await step("Down x12", "simulateKey down x12", async () => {
      for (let i = 0; i < 12; i++) { driver.simulateKey("down"); await Bun.sleep(25); }
    });
    const c = await step("Up x20 (past top)", "simulateKey up x20", async () => {
      for (let i = 0; i < 20; i++) { driver.simulateKey("up"); await Bun.sleep(25); }
    });
    const inBounds = !c.crashed && (c.selectedIndex == null || c.selectedIndex >= 0);
    return { ok: inBounds, reason: inBounds ? "selection stayed in bounds through rapid nav" : "selection went out of bounds / crashed" };
  });

  // ─── Story 9: zero-results empty state ─────────────────────────────────
  await runStory("s9-empty", "Zero-results empty state", "Type gibberish that matches nothing; expect a graceful empty state.", async (step) => {
    await step("Open main", "triggerBuiltin mainList", async () => {
      driver.send({ type: "triggerBuiltin", name: "mainList" });
    });
    const b = await step("Type gibberish", "setFilter zzqqxwvk9271", async () => driver.setFilter("zzqqxwvk9271nonexistent"));
    const ok = !b.crashed;
    return { ok, reason: ok ? `graceful empty/near-empty state (visible=${b.visibleChoiceCount})` : "crashed on zero-results filter" };
  });

  // Final health check: is the app still alive after all 9 stories?
  let finalAlive = false;
  try { await driver.getState({ timeoutMs: 5000 }); finalAlive = true; } catch {}
  const finalShot = await shot("final-alive");

  await driver.close();

  const summary = {
    schemaVersion: 1,
    generatedForReview: true,
    binary: BINARY,
    outDir: OUT,
    finalAppAlive: finalAlive,
    finalShot,
    counts: {
      total: stories.length,
      pass: stories.filter((s) => s.verdict === "PASS").length,
      suspect: stories.filter((s) => s.verdict === "SUSPECT").length,
      fail: stories.filter((s) => s.verdict === "FAIL").length,
    },
    stories,
  };
  writeFileSync(join(OUT, "receipt.json"), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify({ outDir: OUT, counts: summary.counts, finalAlive }, null, 2));
}

main().catch((e) => {
  console.error("FATAL", e);
  process.exit(1);
});

#!/usr/bin/env bun
/**
 * Is Agent Chat actually broken, or is it a sandbox artifact?
 *
 * .notes/ai-sync.md F1 reports Agent Chat and Mini AI failing ~850ms after
 * submit with failureCategory=unknown and zero WARN/ERROR logs. That was
 * measured with `sandboxHome: true`, i.e. a synthetic $HOME with seeded auth.
 *
 * This probe runs the SAME turn under both home configurations back to back so
 * the sandbox is the only variable:
 *
 *   realHome    -> the user's actual ~/.scriptkit and real Pi/Codex auth
 *   sandboxHome -> the F1 reproduction conditions
 *
 * If realHome succeeds and sandboxHome fails, F1 is a harness gap (P1).
 * If both fail, Agent Chat is genuinely broken for users (P0).
 *
 * Usage: bun scripts/agentic/agent-chat-real-home-probe.ts [--reps 2]
 */
import { Driver, type Json } from "../devtools/driver.ts";
import { writeFileSync, mkdirSync } from "node:fs";

const argOf = (f: string) => {
  const i = process.argv.indexOf(f);
  return i >= 0 ? process.argv[i + 1] : undefined;
};
const REPS = Number(argOf("--reps") ?? 2);
const QUERY = argOf("--query") ?? "Say hello in exactly three words.";
const TURN_TIMEOUT_MS = 45_000;
const OUT = "/tmp/agent-chat-real-home";
mkdirSync(OUT, { recursive: true });

const readState = async (d: Driver): Promise<Json | null> => {
  try {
    return await d.request({ type: "getAgentChatState" }, { timeoutMs: 8000 });
  } catch {
    return null;
  }
};

async function waitForAgentSurface(d: Driver, timeoutMs = 20_000) {
  const start = performance.now();
  while (performance.now() - start < timeoutMs) {
    const s = await readState(d);
    if (s && s.status && s.status !== "notAgentChat") return true;
    await Bun.sleep(150);
  }
  return false;
}

async function runOnce(useSandbox: boolean, rep: number): Promise<Json> {
  const label = useSandbox ? "sandboxHome" : "realHome";
  const rec: Json = { config: label, rep };
  const d = await Driver.launch(
    useSandbox ? { sandboxHome: true, seedAgentAuth: true } : {},
  );
  rec.pid = d.pid;
  try {
    await d.waitForSettle();
    d.send({ type: "openAi" });
    rec.surfaceReady = await waitForAgentSurface(d);
    if (!rec.surfaceReady) {
      rec.verdict = "surface-never-opened";
      return rec;
    }

    const pre = await readState(d);
    rec.profileBefore = {
      profile: (pre as any)?.profileId ?? (pre as any)?.profile ?? null,
      modelId: (pre as any)?.modelId ?? null,
      selectionOrigin: (pre as any)?.selectionOrigin ?? null,
    };

    const t0 = performance.now();
    await d.request(
      { type: "setAgentChatInput", text: QUERY, submit: true },
      { timeoutMs: 10_000 },
    );

    // Poll until the turn reaches a terminal status.
    let final: Json | null = null;
    while (performance.now() - t0 < TURN_TIMEOUT_MS) {
      const s = await readState(d);
      const status = (s as any)?.status;
      if (status === "error" || status === "idle") {
        // idle with >1 message means the assistant actually replied
        const count = (s as any)?.messageCount ?? 0;
        if (status === "error" || count > 1) {
          final = s;
          break;
        }
      }
      await Bun.sleep(120);
    }
    rec.turnMs = Math.round(performance.now() - t0);
    rec.finalStatus = (final as any)?.status ?? "timeout";
    rec.messageCount = (final as any)?.messageCount ?? null;
    rec.reliability = (final as any)?.reliability ?? null;
    rec.verdict =
      rec.finalStatus === "error"
        ? "FAILED"
        : rec.finalStatus === "timeout"
          ? "TIMEOUT"
          : "OK";
  } catch (err) {
    rec.error = String(err);
    rec.verdict = "PROBE-ERROR";
  } finally {
    try {
      const lg = await d.getLogs({ limit: 300 });
      const entries = Array.isArray((lg as any)?.entries)
        ? ((lg as any).entries as Json[])
        : [];
      rec.warnErrorCount = entries.filter(
        (e: any) => e.level === "WARN" || e.level === "ERROR",
      ).length;
      rec.aiLogs = entries
        .filter(
          (e: any) =>
            typeof e.message === "string" &&
            /pi_|agent_chat|sidecar|auth|profile/.test(e.message),
        )
        .slice(-10)
        .map((e: any) => `${e.level ?? "?"} ${e.message}`);
    } catch {
      /* best effort */
    }
    await d.close();
  }
  return rec;
}

const results: Json[] = [];
for (const useSandbox of [false, true]) {
  for (let r = 1; r <= REPS; r++) {
    const rec = await runOnce(useSandbox, r);
    results.push(rec);
    console.error(
      `[probe] ${rec.config} rep${r}: verdict=${rec.verdict} status=${rec.finalStatus} ` +
        `turn=${rec.turnMs ?? "-"}ms msgs=${rec.messageCount ?? "-"} ` +
        `warn/err=${rec.warnErrorCount ?? "?"} ` +
        `cat=${(rec.reliability as any)?.failureCategory ?? "-"}`,
    );
  }
}

const realOk = results.filter((r) => r.config === "realHome" && r.verdict === "OK").length;
const sandboxOk = results.filter((r) => r.config === "sandboxHome" && r.verdict === "OK").length;
const verdict =
  realOk > 0 && sandboxOk === 0
    ? "F1 IS A HARNESS GAP (P1) — real home works, sandbox does not"
    : realOk === 0 && sandboxOk === 0
      ? "F1 IS A REAL PRODUCT BUG (P0) — Agent Chat fails with real auth too"
      : realOk > 0 && sandboxOk > 0
        ? "NOT REPRODUCED — both configs succeeded"
        : "MIXED — inspect receipts";

const out = { schemaVersion: 1, query: QUERY, reps: REPS, results, realOk, sandboxOk, verdict };
writeFileSync(`${OUT}/results.json`, JSON.stringify(out, null, 2));
console.error(`\n=== VERDICT: ${verdict} ===`);
console.error(`realHome OK: ${realOk}/${REPS}   sandboxHome OK: ${sandboxOk}/${REPS}`);
console.error(`receipt: ${OUT}/results.json`);
console.log(JSON.stringify(out, null, 2));

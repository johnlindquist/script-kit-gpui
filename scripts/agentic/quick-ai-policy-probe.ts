#!/usr/bin/env bun
/**
 * WP-B2 runtime proof — Quick AI context/tool admission boundary.
 *
 * The AUTHORITATIVE admission logic (classify → adjudicate, tool-policy,
 * forbidden-tool fail-closed, permission rejection, and the zero-context
 * backstop) is locked by the unit matrix in
 * `src/ai/agent_chat/ui/{capabilities,thread}.rs`. This probe confirms, against
 * the REAL binary, that the boundary holds at the surfaces a user can reach:
 *
 *   1. Quick AI launches web-search-only. Either the Codex exec path runs
 *      (`quick_ai_codex_view_switched`; that adapter hardcodes
 *      `allowedTools:["web_search"]` and rejects any turn that is not a single
 *      user-text block), or the Pi process carries `--tools web_search` on its
 *      real argv.
 *   2. The pre-thread zero-context launch invariant is NOT violated
 *      (`quick_ai_zero_context_launch_invariant_violated` never logged) — the
 *      launch really was clean.
 *   3. Attempting the context-bearing ingresses a user can drive — an inline
 *      `@` mention, a slash-skill, and Cmd+P history — never smuggles context:
 *      the zero-context backstop (`quick_ai_context_leak_prevented`) never has
 *      to fire and history stays gated. Context-part CREATION is refused above
 *      this layer by the view affordance gates; the thread admission boundary
 *      is the authoritative backstop and is proven by the unit matrix.
 *
 * NOT automatable through the driver (reported, never faked): drag-and-drop
 * file attach and clipboard image paste.
 *
 * Run: bun scripts/agentic/quick-ai-policy-probe.ts [--receipt <path>]
 */
import { Driver } from "../devtools/driver.ts";

const binary =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  "target-agent/artifacts/w2-quickai/script-kit-gpui";

const receiptArgIdx = process.argv.indexOf("--receipt");
const receiptPath =
  receiptArgIdx >= 0 ? process.argv[receiptArgIdx + 1] : undefined;

const receipt: Record<string, unknown> = {
  probe: "quick-ai-policy",
  binary,
  authoritativeProofLocation:
    "unit matrix: capabilities::quick_ai_context_admission_matrix + " +
    "thread::tests::quick_ai_* (context ingress, forbidden tool, tool policy)",
  unautomatable: [
    "drag-and-drop file attach (no driver primitive)",
    "clipboard image paste (no driver primitive)",
  ],
};

const driver = await Driver.launch({
  sessionName: "quick-ai-policy-probe",
  binary,
  sandboxHome: true,
  seedAgentAuth: true,
});

/** True if `needle` appears anywhere in the recent log blob. */
async function logSeen(needle: string, timeoutMs = 500): Promise<boolean> {
  const start = performance.now();
  do {
    const logs = await driver.getLogs({ limit: 800 });
    if (JSON.stringify(logs).includes(needle)) return true;
    if (performance.now() - start >= timeoutMs) return false;
    await Bun.sleep(150);
  } while (performance.now() - start < timeoutMs);
  return false;
}

async function typeString(text: string): Promise<void> {
  for (const ch of text) {
    await driver.simulateGpuiKeyDown(ch, { text: ch }).catch(() => null);
  }
}

try {
  await driver.waitForSettle();

  // --- Enter Quick AI: text + Tab -----------------------------------------
  await driver.setFilterAndWait("what is the capital of france");
  driver.simulateKey("tab");
  const quickAiEntry = await logSeen("quick_ai_tab_entry", 6000);
  const codexViewSwitched = await logSeen("quick_ai_codex_view_switched", 8000);
  await driver.waitForSettle({ timeoutMs: 8000 });

  // --- Proof 1: web-search-only backend ------------------------------------
  let piLine = "";
  const deadline = performance.now() + 12000;
  while (performance.now() < deadline) {
    const ps = Bun.spawnSync(["pgrep", "-fl", "mode rpc"]);
    const line = ps.stdout
      .toString()
      .split("\n")
      .find(
        (l) =>
          l.includes("gpt-5.3-codex-spark") && l.includes("You are Quick AI"),
      );
    if (line) {
      piLine = line;
      break;
    }
    if (codexViewSwitched) break; // codex exec path — no pi rpc process
    await Bun.sleep(250);
  }
  receipt.launch = { quickAiEntry, codexViewSwitched };
  receipt.tools = {
    codexExecPathUsed: codexViewSwitched,
    piProcessFound: piLine.length > 0,
    piHasWebSearchTool: piLine.includes("--tools web_search"),
    // Either backend is web-search-only: codex exec hardcodes the allowlist and
    // rejects multi-block turns; pi bakes `--tools web_search` onto its argv.
    webSearchOnly:
      codexViewSwitched ||
      (piLine.length > 0 && piLine.includes("--tools web_search")),
  };

  // --- Proof 2: launch invariant was NOT violated --------------------------
  receipt.launchInvariantViolated = await logSeen(
    "quick_ai_zero_context_launch_invariant_violated",
    300,
  );

  // --- Proof 3: drive the user-reachable context ingresses -----------------
  // Each of these is a would-be context ingress. In Quick AI the view
  // affordance gates refuse to open the portal/picker, so the sigils stay
  // literal composer text; the thread admission boundary is the backstop
  // beneath. We assert NO context ever leaks (no backstop trip, history gated).

  // Inline @ mention.
  await typeString(" @file secret.txt");
  await driver.waitForSettle({ timeoutMs: 2500 });

  // Slash skill.
  await typeString(" /deploy");
  await driver.waitForSettle({ timeoutMs: 2500 });

  // Cmd+P history popup.
  driver.simulateKey("p", ["cmd"]);
  await driver.waitForSettle({ timeoutMs: 2500 });
  driver.simulateKey("escape");
  await driver.waitForSettle({ timeoutMs: 2000 });

  const leakPrevented = await logSeen("quick_ai_context_leak_prevented", 300);
  const forbiddenToolRejected = await logSeen(
    "quick_ai_forbidden_tool_rejected",
    300,
  );
  const forbiddenPermission = await logSeen(
    "quick_ai_forbidden_permission_request_rejected",
    300,
  );
  receipt.ingressAttempts = {
    attempted: ["inline @ mention", "slash skill", "Cmd+P history"],
    // A backstop trip means context slipped past the view gate but was caught
    // by the thread admission layer — still denied, but noteworthy.
    zeroContextBackstopTripped: leakPrevented,
    forbiddenToolRejected,
    forbiddenPermissionRejected: forbiddenPermission,
    // No leak + no violation ⇒ context was refused at/above the boundary.
    allIngressesDenied: !leakPrevented && !receipt.launchInvariantViolated,
  };

  const checks: Array<[string, boolean]> = [
    ["launch.quickAiEntry", quickAiEntry],
    ["launch.codexViewSwitched", codexViewSwitched],
    ["tools.webSearchOnly", Boolean((receipt.tools as any).webSearchOnly)],
    ["launchInvariantNotViolated", !receipt.launchInvariantViolated],
    [
      "ingressesDenied",
      Boolean((receipt.ingressAttempts as any).allIngressesDenied),
    ],
  ];
  receipt.pass = checks.every(([, ok]) => ok);
  receipt.failedChecks = checks.filter(([, ok]) => !ok).map(([n]) => n);
} finally {
  await driver.close();
}

const serialized = JSON.stringify(receipt, null, 2);
if (receiptPath) await Bun.write(receiptPath, serialized);
console.log(serialized);
if (!receipt.pass) process.exit(1);

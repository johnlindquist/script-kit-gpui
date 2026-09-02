#!/usr/bin/env bun
/**
 * Independent validation of pi's eyeline Phase-C measurability fix
 * (manager round-61): shared builtin leading separator paints under stable
 * selector `builtin-leading-separator` and appears in getLayoutInfo with
 * non-zero window-relative bounds; firstRowY = separator.y + separator.height
 * matches the rendered first list row (ListItem[0]).
 *
 * Hidden-window only. Publish and name the exact artifact:
 *   bun scripts/devtools/devtools.ts build-ops act app-build --artifact-out .test-output/measurability.reference.json
 *   SCRIPT_KIT_ARTIFACT_REFERENCE=.test-output/measurability.reference.json \
 *     bun scripts/agentic/measurability-builtin-separator-validate.ts
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Driver, type Json } from "../devtools/driver";
import { runtimeArtifactFromEnvironment } from "../devtools/lib/runtime-task-proof.ts";

const artifact = runtimeArtifactFromEnvironment();
const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY ?? artifact.executablePath;
const OUT = join(
  process.cwd(),
  process.env.MEASURABILITY_OUT ?? ".test-output/measurability-builtin-separator",
);
mkdirSync(OUT, { recursive: true });

const EPS = Number(process.env.MEASURABILITY_EPS ?? "1.0");
const SEPARATOR_ID = "builtin-leading-separator";

// 2–3 builtin browsers that use the shared leading separator (pi inventory).
const SURFACES: Array<{ id: string; trigger: string; view: string }> = [
  { id: "clipboard", trigger: "clipboardHistory", view: "clipboardHistory" },
  { id: "apps", trigger: "apps", view: "apps" },
  { id: "settings", trigger: "settings", view: "settings" },
];

type Bounds = { x: number; y: number; width: number; height: number };

function boundsOf(c: Json): Bounds | null {
  const b = c?.bounds;
  if (!b || typeof b.y !== "number" || typeof b.x !== "number") return null;
  if (typeof b.width !== "number" || typeof b.height !== "number") return null;
  return { x: b.x, y: b.y, width: b.width, height: b.height };
}

async function settle(d: Driver, ms = 400) {
  await Bun.sleep(ms);
  try {
    await d.waitForSettle({ timeoutMs: 6000 });
  } catch {}
}

async function openBuiltin(d: Driver, trigger: string, view: string): Promise<{ ok: boolean; promptType: string | null }> {
  d.send({ type: "triggerBuiltin", name: trigger });
  const start = performance.now();
  while (performance.now() - start < 8000) {
    const st: Json = await d.getState({ timeoutMs: 8000 });
    const pt = st?.promptType != null ? String(st.promptType) : null;
    if (pt === view || (pt && pt.toLowerCase().includes(view.toLowerCase()))) {
      return { ok: true, promptType: pt };
    }
    // some surfaces use PascalCase promptType
    if (pt && pt.replace(/[^a-zA-Z]/g, "").toLowerCase() === view.replace(/[^a-zA-Z]/g, "").toLowerCase()) {
      return { ok: true, promptType: pt };
    }
    await settle(d, 300);
  }
  const st: Json = await d.getState({ timeoutMs: 5000 }).catch(() => null as any);
  return { ok: false, promptType: st?.promptType != null ? String(st.promptType) : null };
}

async function backToMain(d: Driver) {
  for (let i = 0; i < 4; i++) {
    try {
      d.simulateKey("escape");
    } catch {}
    await settle(d, 200);
    const st: Json = await d.getState({ timeoutMs: 5000 }).catch(() => null as any);
    const pt = String(st?.promptType ?? "none");
    if (pt === "none" || pt === "" || pt === "ScriptList") return;
  }
}

const receipt: Json = {
  schemaVersion: 1,
  tool: "measurability-builtin-separator-validate",
  binary: BINARY,
  artifact: artifact.reference,
  eps: EPS,
  separatorId: SEPARATOR_ID,
  surfaces: [] as Json[],
  pass: false,
  verdict: null as string | null,
};

const d = await Driver.launch({
  binary: BINARY,
  immutableArtifact: artifact.reference,
  sandboxHome: true,
  sessionName: `measurability-${process.pid}`,
  readyTimeoutMs: 25_000,
  defaultTimeoutMs: 12_000,
});
receipt.sessionDir = d.sessionDir;

try {
  await settle(d, 500);

  for (const surf of SURFACES) {
    await backToMain(d);
    const opened = await openBuiltin(d, surf.trigger, surf.view);
    await settle(d, 500);

    // Nudge results state (empty→results) so separator paints with list
    try {
      d.setFilter("a");
      await settle(d, 350);
    } catch {}

    const layout: Json = await d.getLayoutInfo({}, { timeoutMs: 10_000 });
    const elements: Json = await d
      .getElements({ limit: 80, includeHeaders: true }, { timeoutMs: 10_000 })
      .catch(() => ({ elements: [] }));

    const comps = ((layout?.components ?? []) as Json[])
      .map((c) => ({
        name: String(c?.name ?? ""),
        type: String(c?.type ?? ""),
        bounds: boundsOf(c),
      }))
      .filter((c) => c.bounds != null) as Array<{ name: string; type: string; bounds: Bounds }>;

    const sepComp = comps.find((c) => c.name === SEPARATOR_ID) ?? null;
    const sepBounds = sepComp?.bounds ?? null;
    const sepNonZero =
      sepBounds != null && sepBounds.width > 0 && sepBounds.height > 0;

    const listItems = comps
      .filter((c) => /^ListItem\[\d+\]$/.test(c.name))
      .sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));

    // First *content* row: prefer ListItem whose y is at/after separator bottom;
    // fall back to ListItem[0] if present.
    let firstRow: { name: string; bounds: Bounds } | null = null;
    if (sepBounds && listItems.length) {
      const bottom = sepBounds.y + sepBounds.height;
      const after = listItems.find((c) => c.bounds.y + EPS >= bottom - 0.5);
      // If ListItem[0] is the header itself (y ~= sep.y), take the next item
      if (after && Math.abs(after.bounds.y - sepBounds.y) < EPS && listItems.length > 1) {
        firstRow = { name: listItems[1].name, bounds: listItems[1].bounds };
      } else if (after) {
        firstRow = { name: after.name, bounds: after.bounds };
      } else {
        firstRow = { name: listItems[0].name, bounds: listItems[0].bounds };
      }
    } else if (listItems[0]) {
      firstRow = { name: listItems[0].name, bounds: listItems[0].bounds };
    }

    const predictedFirstRowY =
      sepBounds != null ? sepBounds.y + sepBounds.height : null;
    const actualFirstRowY = firstRow?.bounds.y ?? null;
    const yMatch =
      predictedFirstRowY != null &&
      actualFirstRowY != null &&
      Math.abs(predictedFirstRowY - actualFirstRowY) <= EPS;

    // Semantic presence via getElements (OF-15 / pi plumbing)
    const els = ((elements?.elements ?? []) as Json[]).filter((e) => {
      if (e.type === "input" || e.type === "list") return false;
      if (e.role === "footer") return false;
      return true;
    });
    const firstEl = els[0] ?? null;
    const semanticLeading =
      firstEl != null &&
      (/sectionHeader|leadingSeparator/i.test(
        `${firstEl.role ?? ""} ${firstEl.kind ?? ""} ${firstEl.semanticId ?? ""}`,
      ));

    const surfacePass = Boolean(opened.ok && sepNonZero && yMatch);
    const surface: Json = {
      id: surf.id,
      trigger: surf.trigger,
      expectedView: surf.view,
      opened: opened.ok,
      promptType: opened.promptType,
      separator: {
        found: sepComp != null,
        type: sepComp?.type ?? null,
        bounds: sepBounds,
        nonZero: sepNonZero,
      },
      firstRow: firstRow
        ? { name: firstRow.name, bounds: firstRow.bounds }
        : null,
      predictedFirstRowY,
      actualFirstRowY,
      yDelta:
        predictedFirstRowY != null && actualFirstRowY != null
          ? actualFirstRowY - predictedFirstRowY
          : null,
      yMatch,
      semanticLeading,
      listItemNames: listItems.slice(0, 6).map((c) => c.name),
      componentNamesSample: comps.map((c) => c.name).filter((n) =>
        /separator|header|ListItem|MainView|list/i.test(n),
      ).slice(0, 40),
      pass: surfacePass,
      failReasons: [
        !opened.ok ? "open-failed" : null,
        !sepComp ? "separator-component-missing" : null,
        sepComp && !sepNonZero ? "separator-zero-bounds" : null,
        !firstRow ? "first-row-missing" : null,
        firstRow && !yMatch ? `firstRowY-mismatch delta=${(actualFirstRowY! - predictedFirstRowY!).toFixed(2)}` : null,
      ].filter(Boolean),
    };
    (receipt.surfaces as Json[]).push(surface);
    writeFileSync(join(OUT, `surface-${surf.id}.json`), JSON.stringify(surface, null, 2));

    // clear filter before next
    try {
      d.setFilter("");
    } catch {}
  }

  const allPass = (receipt.surfaces as Json[]).every((s) => s.pass === true);
  const anySep = (receipt.surfaces as Json[]).some((s) => s.separator?.found === true);
  receipt.pass = allPass;
  receipt.verdict = allPass
    ? "ACCEPT — builtin-leading-separator exposed with non-zero bounds; firstRowY = sep.y+sep.h matches ListItem on all probed surfaces"
    : !anySep
      ? "REJECT — builtin-leading-separator not present in getLayoutInfo on probed surfaces"
      : "REJECT — partial: see per-surface failReasons";
} catch (e) {
  receipt.pass = false;
  receipt.verdict = `REJECT — probe error: ${String(e).slice(0, 300)}`;
  receipt.error = String(e);
} finally {
  await d.close();
}

writeFileSync(join(OUT, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");
console.log(
  JSON.stringify(
    {
      pass: receipt.pass,
      verdict: receipt.verdict,
      surfaces: (receipt.surfaces as Json[]).map((s) => ({
        id: s.id,
        pass: s.pass,
        sep: s.separator,
        yDelta: s.yDelta,
        fail: s.failReasons,
      })),
      out: OUT,
    },
    null,
    2,
  ),
);
process.exit(receipt.pass ? 0 : 1);

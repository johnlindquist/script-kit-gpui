#!/usr/bin/env bun
/**
 * Round-62 path adjudication + measurability validation.
 *
 * 3-way contradiction:
 *  - kimi eyeline matrix: leading separator MISSING on builtin browsers
 *  - of15 green: present on same surfaces
 *  - pi OF-18: present on dictationHistory
 *
 * Hypothesis: open-path dependent (name alias vs builtinId; of15 opener vs
 * eyeline opener). Live check file-search + dictation via both openers and
 * extra alias/id paths; also assert paint selector builtin-leading-separator.
 *
 * Verdict:
 *  - path-dependent presence → OF-21 product finding
 *  - same presence all paths, kimi detection wrong → probe bug
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const BINARY =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  join(process.cwd(), "target-agent/artifacts/monkey-grok-measurability/script-kit-gpui");
const OUT = join(
  process.cwd(),
  process.env.PATH_ADJUDICATION_OUT ?? ".test-output/separator-path-adjudication",
);
mkdirSync(OUT, { recursive: true });
const EPS = 1.0;
const SEPARATOR_ID = "builtin-leading-separator";

type Bounds = { x: number; y: number; width: number; height: number };

function boundsOf(c: Json): Bounds | null {
  const b = c?.bounds;
  if (!b || typeof b.y !== "number" || typeof b.width !== "number" || typeof b.height !== "number")
    return null;
  return { x: b.x, y: b.y, width: b.width, height: b.height };
}

async function settle(d: Driver, ms = 350) {
  await Bun.sleep(ms);
  try {
    await d.waitForSettle({ timeoutMs: 5000 });
  } catch {}
}

async function waitView(d: Driver, view: string, ms = 8000): Promise<string | null> {
  const start = performance.now();
  while (performance.now() - start < ms) {
    const st: Json = await d.getState({ timeoutMs: 8000 });
    const pt = st?.promptType != null ? String(st.promptType) : null;
    if (pt === view) return pt;
    await settle(d, 250);
  }
  const st: Json = await d.getState({ timeoutMs: 5000 }).catch(() => null as any);
  return st?.promptType != null ? String(st.promptType) : null;
}

async function backMain(d: Driver) {
  for (let i = 0; i < 5; i++) {
    try {
      d.simulateKey("escape");
    } catch {}
    await settle(d, 150);
    const st: Json = await d.getState({ timeoutMs: 5000 }).catch(() => null as any);
    const pt = String(st?.promptType ?? "none");
    if (pt === "none" || pt === "" || pt === "ScriptList") return;
  }
}

/** OF-15 style first-row detection (role/kind/semanticId sectionHeader). */
function of15Leading(elements: Json) {
  const els: Json[] = (elements?.elements ?? []) as Json[];
  const rows = els.filter((e) => {
    if (e.semanticId === "input:filter" || e.semanticId === "list:results") return false;
    if (e.type === "input" || e.type === "list") return false;
    if (e.role === "footer") return false;
    return true;
  });
  if (rows.length === 0) return { present: false, reason: "empty-list", first: null as Json | null, headerCount: 0 };
  const first = rows[0];
  const present =
    first.role === "sectionHeader" ||
    first.kind === "sectionHeader" ||
    /sectionHeader|section:/.test(String(first.semanticId ?? ""));
  return {
    present,
    reason: present ? "first-row-sectionHeader" : "first-row-not-sectionHeader",
    first: {
      semanticId: first.semanticId ?? null,
      role: first.role ?? null,
      kind: first.kind ?? null,
      type: first.type ?? null,
      text: typeof first.text === "string" ? first.text.slice(0, 80) : null,
    },
    headerCount: rows.filter((r) => r.role === "sectionHeader" || r.kind === "leadingSeparator").length,
  };
}

/** Eyeline style: also accepts leadingSeparator in hay. */
function eyelineLeading(elements: Json) {
  const els: Json[] = (elements?.elements ?? []) as Json[];
  const rows = els.filter((e) => {
    if (e.type === "input" || e.type === "list") return false;
    if (e.role === "footer") return false;
    return true;
  });
  if (rows.length === 0) return { present: false, reason: "empty-list", first: null as Json | null };
  const first = rows[0];
  const hay = `${first.role ?? ""} ${first.kind ?? ""} ${first.semanticId ?? ""}`;
  const present = /sectionHeader|leadingSeparator|section:/i.test(hay);
  return {
    present,
    reason: present ? "eyeline-match" : "eyeline-miss",
    first: {
      semanticId: first.semanticId ?? null,
      role: first.role ?? null,
      kind: first.kind ?? null,
      type: first.type ?? null,
      text: typeof first.text === "string" ? first.text.slice(0, 80) : null,
    },
  };
}

async function snap(d: Driver, label: string): Promise<Json> {
  await settle(d, 300);
  const state: Json = await d.getState({ timeoutMs: 10_000 });
  const elements: Json = await d.getElements(
    { limit: 150, includeHeaders: true },
    { timeoutMs: 10_000 },
  );
  const layout: Json = await d.getLayoutInfo({}, { timeoutMs: 10_000 });
  const comps = ((layout?.components ?? []) as Json[])
    .map((c) => ({ name: String(c?.name ?? ""), type: String(c?.type ?? ""), bounds: boundsOf(c) }))
    .filter((c) => c.bounds != null) as Array<{ name: string; type: string; bounds: Bounds }>;

  const paintSep = comps.find((c) => c.name === SEPARATOR_ID) ?? null;
  const paintNonZero =
    paintSep != null && paintSep.bounds.width > 0 && paintSep.bounds.height > 0;

  const listItems = comps
    .filter((c) => /^ListItem\[\d+\]$/.test(c.name))
    .sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
  const firstListItem = listItems[0] ?? null;
  const predicted =
    paintSep != null ? paintSep.bounds.y + paintSep.bounds.height : null;
  // Prefer ListItem after separator bottom if paint exists
  let contentRow = firstListItem;
  if (paintSep && listItems.length) {
    const bottom = paintSep.bounds.y + paintSep.bounds.height;
    const after = listItems.find((c) => c.bounds.y + 0.5 >= bottom - EPS);
    if (after && Math.abs(after.bounds.y - paintSep.bounds.y) < EPS && listItems[1]) {
      contentRow = listItems[1];
    } else if (after) contentRow = after;
  }
  const yMatch =
    predicted != null &&
    contentRow != null &&
    Math.abs(predicted - contentRow.bounds.y) <= EPS;

  const of15 = of15Leading(elements);
  const eye = eyelineLeading(elements);

  return {
    label,
    promptType: state?.promptType ?? null,
    surfaceKind: state?.surfaceContract?.surfaceKind ?? null,
    inputValue: String(
      state?.filterInputDiagnostics?.canonicalFilterText ??
        state?.inputValue ??
        state?.filter ??
        "",
    ).slice(0, 40),
    visibleChoiceCount: state?.visibleChoiceCount ?? null,
    of15,
    eyeline: eye,
    paint: {
      found: paintSep != null,
      type: paintSep?.type ?? null,
      bounds: paintSep?.bounds ?? null,
      nonZero: paintNonZero,
    },
    firstListItem: contentRow
      ? { name: contentRow.name, bounds: contentRow.bounds }
      : null,
    predictedFirstRowY: predicted,
    yMatch,
    semanticAndPaintAgree: of15.present === paintNonZero,
    elementHead: ((elements?.elements ?? []) as Json[]).slice(0, 8).map((e) => ({
      semanticId: e.semanticId ?? null,
      type: e.type ?? null,
      role: e.role ?? null,
      kind: e.kind ?? null,
      text: typeof e.text === "string" ? e.text.slice(0, 40) : null,
    })),
    paintNames: comps.map((c) => c.name).filter((n) =>
      /separator|ListItem|Header|list/i.test(n),
    ).slice(0, 30),
  };
}

type OpenPath = {
  id: string;
  surface: "file-search" | "dictation";
  view: string;
  open: (d: Driver) => Promise<void>;
  filterAfter?: string;
};

const PATHS: OpenPath[] = [
  // —— file-search ——
  {
    id: "file-search/of15-name",
    surface: "file-search",
    view: "fileSearch",
    // OF-15 primary: triggerBuiltin name, wait view, optional filter
    open: async (d) => {
      d.send({ type: "triggerBuiltin", name: "files" });
      await waitView(d, "fileSearch", 8000);
    },
    filterAfter: "abc",
  },
  {
    id: "file-search/eyeline-name",
    surface: "file-search",
    view: "fileSearch",
    // Eyeline openBuiltin: trigger only + poll view (no filter until cell)
    open: async (d) => {
      d.send({ type: "triggerBuiltin", name: "files" });
      const start = performance.now();
      while (performance.now() - start < 7000) {
        const st: Json = await d.getState({ timeoutMs: 8000 });
        if (String(st.promptType ?? "") === "fileSearch") return;
        await settle(d, 600);
      }
    },
    filterAfter: "a",
  },
  {
    id: "file-search/builtinId",
    surface: "file-search",
    view: "fileSearch",
    open: async (d) => {
      d.send({ type: "triggerBuiltin", builtinId: "builtin/file-search" });
      await waitView(d, "fileSearch", 8000);
    },
    filterAfter: "a",
  },
  {
    id: "file-search/of15-fallback-enter",
    surface: "file-search",
    view: "fileSearch",
    // OF-15 fallback when trigger misses: type alias + enter
    open: async (d) => {
      d.setFilter("files");
      await settle(d, 400);
      d.simulateKey("enter");
      await waitView(d, "fileSearch", 8000);
    },
    filterAfter: "abc",
  },
  // —— dictation history ——
  {
    id: "dictation/of15-name",
    surface: "dictation",
    view: "dictationHistory",
    open: async (d) => {
      d.send({ type: "triggerBuiltin", name: "dictationHistory" });
      await waitView(d, "dictationHistory", 8000);
    },
    filterAfter: "",
  },
  {
    id: "dictation/eyeline-name",
    surface: "dictation",
    view: "dictationHistory",
    open: async (d) => {
      d.send({ type: "triggerBuiltin", name: "dictationHistory" });
      const start = performance.now();
      while (performance.now() - start < 7000) {
        const st: Json = await d.getState({ timeoutMs: 8000 });
        if (String(st.promptType ?? "") === "dictationHistory") return;
        await settle(d, 600);
      }
    },
    filterAfter: "a",
  },
  {
    id: "dictation/builtinId",
    surface: "dictation",
    view: "dictationHistory",
    open: async (d) => {
      d.send({ type: "triggerBuiltin", builtinId: "builtin/dictation-history" });
      await waitView(d, "dictationHistory", 8000);
    },
    filterAfter: "a",
  },
  {
    id: "dictation/alias-dictation",
    surface: "dictation",
    view: "dictationHistory",
    // Legacy alias name:"dictation" → DictationHistory (not start dictation)
    open: async (d) => {
      d.send({ type: "triggerBuiltin", name: "dictation" });
      await waitView(d, "dictationHistory", 8000);
    },
    filterAfter: "a",
  },
  {
    id: "dictation/of15-fallback-enter",
    surface: "dictation",
    view: "dictationHistory",
    open: async (d) => {
      d.setFilter("dictationHistory");
      await settle(d, 400);
      d.simulateKey("enter");
      await waitView(d, "dictationHistory", 8000);
    },
    filterAfter: "",
  },
];

const receipt: Json = {
  schemaVersion: 1,
  tool: "separator-path-adjudication",
  binary: BINARY,
  paths: [] as Json[],
  adjudication: null as Json,
};

if (!(await Bun.file(BINARY).exists())) {
  console.error(JSON.stringify({ ok: false, error: `binary missing: ${BINARY}` }));
  process.exit(2);
}

const d = await Driver.launch({
  binary: BINARY,
  sandboxHome: true,
  sessionName: `sep-path-${process.pid}`,
  readyTimeoutMs: 25_000,
  defaultTimeoutMs: 12_000,
});
receipt.sessionDir = d.sessionDir;

try {
  await settle(d, 500);

  for (const path of PATHS) {
    await backMain(d);
    let openError: string | null = null;
    try {
      await path.open(d);
    } catch (e) {
      openError = String(e);
    }
    const pt = await waitView(d, path.view, 2000);
    const opened = pt === path.view;

    // empty/default snap
    const snapEmpty = await snap(d, `${path.id}:pre-filter`);

    // post-filter (results state) when requested
    let snapFilter: Json | null = null;
    if (path.filterAfter !== undefined) {
      d.setFilter(path.filterAfter);
      await settle(d, 400);
      snapFilter = await snap(d, `${path.id}:filter=${path.filterAfter}`);
    }

    const row: Json = {
      pathId: path.id,
      surface: path.surface,
      expectedView: path.view,
      opened,
      promptType: pt,
      openError,
      empty: snapEmpty,
      filtered: snapFilter,
      // convenience booleans for adjudication
      of15PresentEmpty: snapEmpty.of15?.present === true,
      eyelinePresentEmpty: snapEmpty.eyeline?.present === true,
      paintPresentEmpty: snapEmpty.paint?.nonZero === true,
      of15PresentFiltered: snapFilter?.of15?.present === true,
      eyelinePresentFiltered: snapFilter?.eyeline?.present === true,
      paintPresentFiltered: snapFilter?.paint?.nonZero === true,
    };
    (receipt.paths as Json[]).push(row);
    writeFileSync(join(OUT, `${path.id.replace(/\//g, "__")}.json`), JSON.stringify(row, null, 2));
    console.error(
      JSON.stringify({
        path: path.id,
        opened,
        of15: row.of15PresentEmpty,
        eye: row.eyelinePresentEmpty,
        paint: row.paintPresentEmpty,
        of15F: row.of15PresentFiltered,
        eyeF: row.eyelinePresentFiltered,
        paintF: row.paintPresentFiltered,
      }),
    );
  }

  // Adjudicate
  const filePaths = (receipt.paths as Json[]).filter((p) => p.surface === "file-search");
  const dictPaths = (receipt.paths as Json[]).filter((p) => p.surface === "dictation");

  function presenceVector(paths: Json[], key: "of15PresentFiltered" | "eyelinePresentFiltered" | "paintPresentFiltered" | "of15PresentEmpty" | "eyelinePresentEmpty" | "paintPresentEmpty") {
    return paths.map((p) => ({ id: p.pathId, v: p[key] === true, opened: p.opened }));
  }

  const fileEye = presenceVector(filePaths, "eyelinePresentFiltered");
  const fileOf15 = presenceVector(filePaths, "of15PresentFiltered");
  const filePaint = presenceVector(filePaths, "paintPresentFiltered");
  const dictEye = presenceVector(dictPaths, "eyelinePresentFiltered");
  const dictOf15 = presenceVector(dictPaths, "of15PresentFiltered");
  const dictPaint = presenceVector(dictPaths, "paintPresentFiltered");

  const openedPaths = (receipt.paths as Json[]).filter((p) => p.opened);
  const eyeVals = openedPaths.map((p) => p.eyelinePresentFiltered === true || (p.filtered == null && p.eyelinePresentEmpty === true));
  const of15Vals = openedPaths.map((p) => p.of15PresentFiltered === true || (p.filtered == null && p.of15PresentEmpty === true));
  const paintVals = openedPaths.map((p) => p.paintPresentFiltered === true || (p.filtered == null && p.paintPresentEmpty === true));

  // Prefer filtered when present
  function effectivePresent(p: Json): { of15: boolean; eye: boolean; paint: boolean } {
    if (p.filtered) {
      return {
        of15: p.of15PresentFiltered === true,
        eye: p.eyelinePresentFiltered === true,
        paint: p.paintPresentFiltered === true,
      };
    }
    return {
      of15: p.of15PresentEmpty === true,
      eye: p.eyelinePresentEmpty === true,
      paint: p.paintPresentEmpty === true,
    };
  }

  const eff = openedPaths.map((p) => ({ id: p.pathId, ...effectivePresent(p), opened: true }));
  const eyeSet = new Set(eff.filter((e) => e.eye).map((e) => e.id));
  const missEye = eff.filter((e) => !e.eye).map((e) => e.id);
  const hitEye = eff.filter((e) => e.eye).map((e) => e.id);
  const pathDependentSemantic =
    hitEye.length > 0 && missEye.length > 0;
  const allEyeMiss = hitEye.length === 0 && eff.length > 0;
  const allEyeHit = missEye.length === 0 && eff.length > 0;
  const paintAlways = eff.every((e) => e.paint);
  const paintNever = eff.every((e) => !e.paint);
  const paintPathDep = !paintAlways && !paintNever && eff.some((e) => e.paint);

  // Detector disagreement: of15 vs eyeline on same snap
  const detectorDisagreement = openedPaths.filter((p) => {
    const e = effectivePresent(p);
    return e.of15 !== e.eye;
  }).map((p) => p.pathId);

  // of15 vs eyeline open paths for same surface — same trigger names mostly;
  // path dep on alias/builtinId/fallback is the interesting OF-21 signal
  let fork: string;
  let summary: string;
  if (pathDependentSemantic || paintPathDep) {
    fork = "OF-21_path_dependent";
    summary =
      "Separator presence differs across open paths — product path-dependent finding OF-21. " +
      `semantic eye hit=[${hitEye.join(",")}] miss=[${missEye.join(",")}]; ` +
      `paint pathDep=${paintPathDep} always=${paintAlways}.`;
  } else if (allEyeMiss && paintAlways) {
    fork = "probe_bug_kimi_filtering";
    summary =
      "Paint selector always present with non-zero bounds, but semantic first-row detection " +
      "misses on all paths — kimi/of15 element-head filtering is the bug, not missing chrome.";
  } else if (allEyeMiss && paintNever) {
    fork = "product_missing_separator";
    summary =
      "Neither paint nor semantic separator present on any path — real missing separator, not path-dep.";
  } else if (allEyeHit && paintAlways) {
    fork = "all_present_contradiction_historical";
    summary =
      "Live: separator present (semantic+paint) on all open paths for file-search+dictation. " +
      "kimi matrix MISSING is stale or probe-state-specific; not path-dependent on openers tested.";
  } else {
    fork = "mixed_needs_manager";
    summary = `Mixed: eye allHit=${allEyeHit} allMiss=${allEyeMiss} paintAlways=${paintAlways} detectorDisagreement=${detectorDisagreement.length}`;
  }

  receipt.adjudication = {
    fork,
    summary,
    effective: eff,
    detectorDisagreement,
    file: { eye: fileEye, of15: fileOf15, paint: filePaint },
    dictation: { eye: dictEye, of15: dictOf15, paint: dictPaint },
    measurability: {
      note: "paint selector builtin-leading-separator non-zero + firstRowY=sep.y+sep.h",
      perPath: openedPaths.map((p) => ({
        id: p.pathId,
        paint: (p.filtered ?? p.empty)?.paint,
        yMatch: (p.filtered ?? p.empty)?.yMatch,
        predictedFirstRowY: (p.filtered ?? p.empty)?.predictedFirstRowY,
        firstListItem: (p.filtered ?? p.empty)?.firstListItem,
      })),
    },
  };
} finally {
  await d.close();
}

writeFileSync(join(OUT, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n");
console.log(JSON.stringify({ fork: receipt.adjudication?.fork, summary: receipt.adjudication?.summary, out: OUT }, null, 2));
process.exit(0);

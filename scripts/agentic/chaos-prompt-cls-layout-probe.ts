#!/usr/bin/env bun
/** NN=22: prompt/builtin chrome CLS across filter storms and data transitions. */
import { join } from "node:path";
import { Driver } from "../devtools/driver";

const BINARY = process.env.SCRIPT_KIT_GPUI_BINARY ?? join(process.cwd(), "target-agent/artifacts/monkey-prompts/script-kit-gpui");
const EPS = 1;
const OF13_ONLY = process.env.OF13_ONLY === "1";
type Bounds = { x: number; y: number; width: number; height: number };
type Snap = { generation: number | null; window: { width: number; height: number }; stable: Record<string, Bounds>; visible: Record<string, Bounds>; rawCount: number };
const rows: any[] = [];
const findings: any[] = [];
const d = await Driver.launch({ sandboxHome: true, binary: BINARY, sessionName: "monkey-x2-prompt-cls-22" });

function stableName(component: any) {
  const hay = `${component?.name ?? ""} ${component?.type ?? ""}`.toLowerCase();
  return ["input", "search", "header", "footer", "hint", "toolbar"].some((part) => hay.includes(part));
}

async function snap(label: string): Promise<Snap> {
  const settled = await d.waitForSettle({ timeoutMs: 4000 }).catch(() => null);
  const info: any = await d.getLayoutInfo({}, { timeoutMs: 6000 });
  const stable: Record<string, Bounds> = {};
  const visible: Record<string, Bounds> = {};
  let paintGeneration: number | null = null;
  for (const component of info?.components ?? []) {
    const bounds = component?.visibleBounds ?? component?.visible_bounds ?? component?.bounds;
    if (!bounds || ![bounds.x, bounds.y, bounds.width, bounds.height].every(Number.isFinite)) continue;
    const key = `${component.name ?? ""}|${component.type ?? ""}`;
    visible[key] = bounds;
    if (stableName(component)) stable[key] = bounds;
    const generation = component?.measurementFrameGeneration ?? component?.measurement_frame_generation;
    if (Number.isFinite(generation)) paintGeneration = Math.max(paintGeneration ?? 0, generation);
  }
  rows.push({ label, settled, generation: paintGeneration, windowWidth: info?.windowWidth, windowHeight: info?.windowHeight, componentCount: (info?.components ?? []).length });
  return { generation: paintGeneration, window: { width: info?.windowWidth ?? 0, height: info?.windowHeight ?? 0 }, stable, visible, rawCount: (info?.components ?? []).length };
}

function compare(surface: string, from: string, to: string, a: Snap, b: Snap) {
  for (const [key, before] of Object.entries(a.stable)) {
    const after = b.stable[key];
    if (!after) continue;
    // Bottom-anchored footer moves by definition when result injection grows
    // the native window. Storm comparisons at a fixed viewport still track it.
    if (a.window.height !== b.window.height && key.toLowerCase().includes("footer")) continue;
    const drift = Math.max(Math.abs(before.x - after.x), Math.abs(before.y - after.y), Math.abs(before.height - after.height));
    if (drift > EPS) findings.push({ severity: "FAIL", surface, lens: "CLS", from, to, key, driftPx: Number(drift.toFixed(2)), before, after });
  }
  for (const [key, bounds] of Object.entries(b.visible)) {
    const clipped = bounds.width < 0 || bounds.height < 0 || bounds.x + bounds.width < 0 || bounds.y + bounds.height < 0;
    if (clipped) findings.push({ severity: "FAIL", surface, lens: "layout", at: to, key, bounds, note: "invalid/interleaved visible bounds" });
  }
}

async function openMessage(surface: string, view: string, message: any) {
  d.send(message);
  const wait: any = await d.waitForState({ promptType: view }, { timeoutMs: 5000 });
  if (!wait?.success) {
    const state: any = await d.getState({ timeoutMs: 5000 });
    if (state?.promptType !== view) findings.push({ severity: "FAIL", surface, lens: "open", wait, observedPromptType: state?.promptType });
  }
}

async function storm(surface: string) {
  let previous = await snap(`${surface}:storm:0`);
  let previousLabel = "0";
  const alphabet = "abcdefghijklmno";
  const queries = [...alphabet].map((_, index) => alphabet.slice(0, index + 1));
  queries.push(...[...queries].reverse());
  for (let index = 0; index < 30; index++) {
    const query = queries[index];
    // setFilter is FIFO with the following getState/getLayoutInfo requests.
    // Do not use setFilterAndWait here: entity-owned Select stores its query
    // outside StateResult.inputValue, so that helper waits on the wrong field.
    d.setFilter(query);
    const current = await snap(`${surface}:storm:${index + 1}`);
    compare(surface, previousLabel, String(index + 1), previous, current);
    previous = current;
    previousLabel = String(index + 1);
  }
}

async function dismiss() {
  d.simulateKey("escape");
  await d.waitForSettle({ timeoutMs: 4000 }).catch(() => null);
  d.send({ type: "triggerBuiltin", name: "mainList" });
  await d.waitForState({ promptType: "none" }, { timeoutMs: 4000 }).catch(() => null);
}

async function runOf13SelectRows() {
  // One choice is the critical content-aware sizing edge: the generic
  // ArgPromptWithChoices height is 119px, but Select also owns an internal
  // search/selection header above its Comfortable (40px) unified row.
  const choices = Array.from({ length: 1 }, (_, index) => ({
    name: `OF-13 choice ${index + 1}`,
    value: `of13-${index + 1}`,
  }));
  const message = {
    type: "select",
    id: "of13-select-collapse",
    placeholder: "~/.scriptkit · Brain · GPT-5.6 SOL",
    choices,
    multiple: true,
  };

  await openMessage("of13-select", "select", message);
  const stateBefore: any = await d.getState({ timeoutMs: 5000 });
  const layoutBefore: any = await d.getLayoutInfo({}, { timeoutMs: 6000 });
  const elementsBefore: any = await d.getElements({ limit: 100 }, { timeoutMs: 5000 });
  const boundedComponents = (layoutBefore.components ?? [])
    .filter((component: any) => component.bounds || component.visibleBounds || component.visible_bounds)
    .map((component: any) => ({
      name: component.name ?? null,
      type: component.type ?? null,
      bounds: component.bounds ?? null,
      visibleBounds: component.visibleBounds ?? component.visible_bounds ?? null,
      generation: component.measurementFrameGeneration ?? component.measurement_frame_generation ?? null,
    }));
  const choiceElements = (elementsBefore.elements ?? []).filter((element: any) => element.type === "choice");
  rows.push({
    label: "of13:collapsed-layout-elements",
    window: { width: layoutBefore.windowWidth, height: layoutBefore.windowHeight },
    promptType: stateBefore.promptType,
    selectedCount: choiceElements.filter((element: any) => element.selected).length,
    footer: stateBefore.activeFooter ?? null,
    boundedComponents,
    elements: elementsBefore,
  });

  const mainHeader = boundedComponents.find((component: any) => component.name === "MainViewHeader");
  const footer = boundedComponents.find((component: any) => component.name === "MainViewFooter");
  const shellContentHeight = footer && mainHeader
    ? footer.bounds.y - (mainHeader.bounds.y + mainHeader.bounds.height)
    : null;
  if (layoutBefore.windowHeight <= 119 || (Number.isFinite(shellContentHeight) && shellContentHeight < 88)) {
    findings.push({
      severity: "FAIL",
      surface: "select",
      lens: "OF-13 sizing",
      windowHeight: layoutBefore.windowHeight,
      shellContentHeight,
      note: "collapsed Select leaves insufficient shell content for its internal search header plus one Comfortable unified row",
    });
  }

  const errorsBefore: any = await d.getLogs({ level: "error", limit: 300 }, { timeoutMs: 5000 });
  d.simulateKey("enter");
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => null);
  const afterEnter: any = await d.getState({ timeoutMs: 5000 });
  const elementsAfterEnter: any = await d.getElements({ limit: 100 }, { timeoutMs: 5000 });
  const logsAfterEnter: any = await d.getLogs({ limit: 120 }, { timeoutMs: 5000 });
  const errorsAfter: any = await d.getLogs({ level: "error", limit: 300 }, { timeoutMs: 5000 });
  rows.push({
    label: "of13:zero-selected-enter",
    before: { promptType: stateBefore.promptType, selectedCount: 0, footer: stateBefore.activeFooter ?? null },
    after: { promptType: afterEnter.promptType, windowVisible: afterEnter.windowVisible },
    footerAfter: afterEnter.activeFooter ?? null,
    elementsAfter: elementsAfterEnter,
    logTail: (logsAfterEnter.entries ?? []).slice(-20),
    newErrors: (errorsAfter.entries ?? []).filter((entry: any) =>
      !(errorsBefore.entries ?? []).some((before: any) => before.target === entry.target && before.message === entry.message)),
  });
  const footerButtons = stateBefore.activeFooter?.buttons ?? [];
  const zeroSelectionIsSafe = afterEnter.promptType === "select"
    && footerButtons.some((button: any) => button.label === "Select one" && button.enabled === false);
  if (!zeroSelectionIsSafe) {
    findings.push({
      severity: "FAIL",
      surface: "select",
      lens: "OF-13 safe default",
      advertisedAction: footerButtons,
      observedAfterEnter: afterEnter.promptType,
      note: "0-selected multi-select must advertise a disabled Select one action and Enter must remain in Select",
    });
  }

  // OF-13d is intentionally isolated from the collapse row: the 18 -> 19
  // character boundary previously changed the measured main header 30 -> 58px.
  const driftMessage = {
    ...message,
    id: "of13-select-header-drift",
    choices: Array.from({ length: 80 }, (_, index) => ({
      name: `OF-13d ${index + 1} abcdefghijklmnopqrstuvwxyz`,
      value: `of13d-${index + 1}`,
    })),
  };
  await openMessage("of13d-select", "select", driftMessage);
  d.setFilter("abcdefghijklmnopqr");
  const layout18: any = await d.getLayoutInfo({}, { timeoutMs: 6000 });
  d.setFilter("abcdefghijklmnopqrs");
  const layout19: any = await d.getLayoutInfo({}, { timeoutMs: 6000 });
  const headerHeight = (layout: any) => (layout.components ?? [])
    .find((component: any) => component.name === "MainViewHeader")?.bounds?.height ?? null;
  const height18 = headerHeight(layout18);
  const height19 = headerHeight(layout19);
  rows.push({
    label: "of13d:header-drift-18-to-19",
    query18: { headerHeight: height18, windowHeight: layout18.windowHeight },
    query19: { headerHeight: height19, windowHeight: layout19.windowHeight },
    delta: Number.isFinite(height18) && Number.isFinite(height19) ? height19 - height18 : null,
  });
  if (height18 !== height19) {
    findings.push({
      severity: "FAIL",
      surface: "select",
      lens: "OF-13d header drift",
      height18,
      height19,
      note: "isolated 18 -> 19 filter query changes the measured main header height",
    });
  }

  d.send({ type: "hide" });
  await d.waitForState({ windowVisible: false }, { timeoutMs: 4000 }).catch(() => null);
  d.send({ type: "show" });
  await d.waitForState({ windowVisible: true }, { timeoutMs: 4000 }).catch(() => null);
  await openMessage("of13-select", "select", { ...message, id: "of13-select-escape" });
  d.simulateKey("escape");
  await d.waitForSettle({ timeoutMs: 3000 }).catch(() => null);
  const afterEscape: any = await d.getState({ timeoutMs: 5000 });
  rows.push({ label: "of13:escape", after: { promptType: afterEscape.promptType, windowVisible: afterEscape.windowVisible } });
  if (afterEscape.promptType === "select" && afterEscape.windowVisible !== false) {
    findings.push({ severity: "FAIL", surface: "select", lens: "OF-13 escape", note: "sessionless Select remained stuck after Escape" });
  }

  d.send({ type: "hide" });
  await d.waitForState({ windowVisible: false }, { timeoutMs: 4000 }).catch(() => null);
  d.send({ type: "show" });
  await d.waitForState({ windowVisible: true }, { timeoutMs: 4000 }).catch(() => null);
}

try {
  d.send({ type: "show" });
  await d.waitForState({ windowVisible: true }, { timeoutMs: 5000 });

  await runOf13SelectRows();

  if (OF13_ONLY) throw new Error("__OF13_FOCUSED_DONE__");

  const promptCases = [
    { id: "arg", view: "arg", small: { type: "arg", id: "cls-arg-small", placeholder: "Arg", choices: [{ name: "One", value: "1" }] }, large: { type: "arg", id: "cls-arg-large", placeholder: "Arg", choices: Array.from({ length: 240 }, (_, i) => ({ name: `Choice ${i}`, value: String(i) })) } },
    { id: "editor", view: "editor", small: { type: "editor", id: "cls-editor-small", content: "one\n", language: "markdown" }, large: { type: "editor", id: "cls-editor-large", content: "line\n".repeat(1200), language: "markdown" } },
    { id: "form", view: "form", small: { type: "form", id: "cls-form-small", html: '<form><input name="a"/></form>' }, large: { type: "form", id: "cls-form-large", html: `<form>${Array.from({ length: 80 }, (_, i) => `<input name="f${i}"/>`).join("")}</form>` } },
    { id: "select", view: "select", small: { type: "select", id: "cls-select-small", placeholder: "Select", choices: [{ name: "One", value: "1" }], multiple: true }, large: { type: "select", id: "cls-select-large", placeholder: "Select", choices: Array.from({ length: 240 }, (_, i) => ({ name: `Choice ${i}`, value: String(i) })), multiple: true } },
  ];

  for (const item of promptCases) {
    await openMessage(item.id, item.view, item.small);
    const small = await snap(`${item.id}:small`);
    if (item.id === "arg" || item.id === "select") await storm(item.id);
    d.send(item.large);
    await d.waitForState({ promptType: item.view }, { timeoutMs: 5000 });
    const large = await snap(`${item.id}:large-injection`);
    compare(item.id, "small", "large-injection", small, large);
    rows.push({ label: `${item.id}:viewport-extremes`, small: small.window, large: large.window, smallGeneration: small.generation, largeGeneration: large.generation });
    await dismiss();
    d.send({ type: "show" });
    await d.waitForState({ windowVisible: true }, { timeoutMs: 5000 });
  }

  for (const builtin of [{ id: "emoji", trigger: "emoji", view: "emojiPicker" }, { id: "file-search", trigger: "files", view: "fileSearch" }]) {
    d.send({ type: "triggerBuiltin", name: builtin.trigger });
    await d.waitForState({ promptType: builtin.view }, { timeoutMs: 5000 });
    const before = await snap(`${builtin.id}:before`);
    await storm(builtin.id);
    const after = await snap(`${builtin.id}:after-storm`);
    compare(builtin.id, "before", "after-storm", before, after);
    await dismiss();
    d.send({ type: "show" });
    await d.waitForState({ windowVisible: true }, { timeoutMs: 5000 });
  }

  for (let index = 0; index < 12; index++) {
    const item = promptCases[index % promptCases.length];
    d.send({ ...item.small, id: `cls-transition-${index}` });
    await d.waitForState({ promptType: item.view }, { timeoutMs: 5000 });
    await snap(`transition:${index}:${item.id}`);
  }

  const errors: any = await d.getLogs({ level: "error", limit: 300 }, { timeoutMs: 5000 });
  const violations: any = await d.getLogs({ target: "script_kit::prompt_chrome", contains: "violation", limit: 300 }, { timeoutMs: 5000 });
  if ((errors.entries ?? []).length) findings.push({ severity: "ERRLOG", entries: errors.entries });
  if ((violations.entries ?? []).length) findings.push({ severity: "FAIL", surface: "hint-audit", entries: violations.entries });
} catch (error) {
  if (!(error instanceof Error) || error.message !== "__OF13_FOCUSED_DONE__") throw error;
} finally {
  d.send({ type: "hide" });
  await d.waitForState({ windowVisible: false }, { timeoutMs: 5000 }).catch(() => null);
  const finalState: any = await d.getState({ timeoutMs: 5000 }).catch(() => ({}));
  console.log(JSON.stringify({ verdict: findings.some((f) => f.severity === "FAIL") ? "FAIL" : findings.length ? "SUSPECT" : "PASS", clsEpsilonPx: EPS, budget: "stable chrome drift 0px expected; >1px fails", findings, rows, finalWindowVisible: finalState.windowVisible, binary: BINARY }, null, 2));
  await d.close();
}

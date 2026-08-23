#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { Driver } from "../../devtools/driver.ts";
import {
  nativeFooterActivationProof,
  semanticAxParity,
  semanticFocusGraph,
} from "../../devtools/focus.ts";
import { analyzeLayout } from "../../devtools/layout.ts";
import { renderedSafeViewportMeasurement } from "../../devtools/scroll.ts";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";
import {
  observeRuntimeTaskTarget,
  prepareBlockedRuntimeTaskProof,
  prepareRuntimeTaskProof,
  type RuntimeTargetObservation,
} from "../../devtools/lib/runtime-task-proof.ts";
import { targetIdentity } from "../../devtools/lib/target-identity.ts";

assertNoninteractiveVisualProbe("cons-proof-gov.ax-scroll");

const binary = resolve(
  process.env.SCRIPT_KIT_GPUI_BINARY
    ?? "target-agent/artifacts/cons-proof-c06/script-kit-gpui",
);
const axPath = resolve(
  process.env.CONSISTENCY_AX_RECEIPT_PATH
    ?? ".artifacts/consistency/PF-007/ax-focus-activation.json",
);
const scrollPath = resolve(
  process.env.CONSISTENCY_SCROLL_RECEIPT_PATH
    ?? ".artifacts/consistency/PF-008/list-scroll.json",
);

type Obj = Record<string, unknown>;

function asObj(value: unknown): Obj {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Obj : {};
}

function asArray(value: unknown): Obj[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is Obj => entry != null && typeof entry === "object" && !Array.isArray(entry))
    : [];
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function fingerprint(value: unknown): string | null {
  if (typeof value !== "string" || value.length === 0) return null;
  return createHash("sha256").update(value).digest("hex");
}

function exactExecutablePids(executable: string): number[] {
  const result = Bun.spawnSync(["ps", "-axo", "pid=,command="], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const normalized = resolve(executable);
  return new TextDecoder().decode(result.stdout)
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .flatMap((line) => {
      const match = line.match(/^(\d+)\s+(.+)$/);
      if (!match) return [];
      const executablePath = match[2].trim().split(/\s+/, 1)[0];
      return resolve(executablePath) === normalized ? [Number(match[1])] : [];
    });
}

async function poll<T>(
  label: string,
  probe: () => Promise<T>,
  predicate: (value: T) => boolean,
  timeoutMs = 12_000,
): Promise<T> {
  const deadline = performance.now() + timeoutMs;
  let last = await probe();
  while (performance.now() < deadline) {
    if (predicate(last)) return last;
    await Bun.sleep(40);
    last = await probe();
  }
  throw new Error(`${label} did not become observable`);
}

function infoOf(layout: Obj): Obj {
  return asObj(layout.info ?? layout);
}

function appKitNodes(layout: Obj): Obj[] {
  const fidelity = asObj(infoOf(layout).fidelity);
  const appkit = asObj(fidelity.appKit ?? fidelity.appkit);
  return asArray(appkit.nodes);
}

async function inspectMain(driver: Driver): Promise<Obj> {
  const windows = asObj(await driver.listAutomationWindows({ timeoutMs: 4_000 }));
  const response = asObj(await driver.request({
    type: "inspectAutomationWindow",
    target: { type: "main" },
    hiDpi: false,
    probes: [],
  }, { expect: "automationInspectResult", timeoutMs: 8_000 }));
  const identity = targetIdentity(
    { target: { type: "main" }, strict: true, expectedSurfaceKind: "" },
    asObj(response.snapshot ?? response),
    windows,
  );
  assert(
    identity.resolvedTarget.strictTargetMatch === true &&
      asArray(identity.resolvedTarget.ambiguity).length === 0,
    "main target identity is ambiguous",
  );
  return identity.resolvedTarget;
}

function targetGeneration(snapshot: Obj) {
  return {
    automationId: snapshot.automationId ?? snapshot.id ?? null,
    windowInstanceId: snapshot.windowInstanceId ?? null,
    targetGeneration: snapshot.targetGeneration ?? null,
    surfaceGeneration: snapshot.surfaceGeneration ?? null,
    dataGeneration: snapshot.dataGeneration ?? null,
  };
}

function targetReceiptFor(layout: Obj): Obj {
  const info = infoOf(layout);
  return {
    resolvedTarget: {
      bounds: {
        x: 0,
        y: 0,
        width: Number(info.windowWidth ?? 0),
        height: Number(info.windowHeight ?? 0),
      },
    },
  };
}

async function waitForFooterLayout(driver: Driver): Promise<Obj> {
  return poll(
    "native footer AX projection",
    async () => asObj(await driver.getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 5_000 })),
    (layout) => appKitNodes(layout).some((node) => node.accessibilityIdentifier === "footer-action:actions"),
  );
}

function cleanupFor(driver: Driver | null, closeError: string | null) {
  return driver
    ? {
        processExited: driver.finalization.processExited,
        streamsDrained: driver.finalization.streamsDrained,
        logWriterClosed: driver.finalization.logWriterClosed,
        ownedProcessCount: exactExecutablePids(binary).length,
        closeError,
        clipboardTouched: false,
      }
    : {
        processExited: false,
        streamsDrained: false,
        logWriterClosed: false,
        ownedProcessCount: exactExecutablePids(binary).length,
        closeError,
        clipboardTouched: false,
      };
}

function cleanupPass(cleanup: ReturnType<typeof cleanupFor>) {
  return cleanup.processExited && cleanup.streamsDrained && cleanup.logWriterClosed &&
    cleanup.ownedProcessCount === 0 && cleanup.closeError == null;
}

async function enabledScenario() {
  let driver: Driver | null = null;
  let closeError: string | null = null;
  let runtimeError: string | null = null;
  let proof: Obj = {};
  let debug: Obj = {};
  let focusObservation: RuntimeTargetObservation | null = null;
  let scrollObservation: RuntimeTargetObservation | null = null;
  let selectedScrollState: Obj = {};
  let focusedSemanticId: string | null = null;
  try {
    driver = await Driver.launch({
      binary,
      sessionName: `cons-proof-c06-enabled-${process.pid}`,
      sandboxHome: true,
      defaultTimeoutMs: 12_000,
      env: {
        SCRIPT_KIT_TEST_STATUS: "1",
        SCRIPT_KIT_FIDELITY_CAPTURE: "agent-chat",
        SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
        SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
      },
    });
    driver.send({ type: "show" });
    await driver.waitForState({ windowVisible: true }, { timeoutMs: 10_000 });

    // The launcher's startup scan regenerates the grouped results for a few
    // seconds after show. Selecting mid-regeneration races the grouping and
    // makes an index-based selection drift to a different row before paint.
    // The rendered-scroll invariant under proof concerns a settled dataset,
    // so wait for the data generation to hold stable before capturing the
    // footer layout or selecting (a pre-settle layout goes stale and fails
    // semantic-to-AX parity spuriously).
    {
      let stableSamples = 0;
      let lastGeneration: unknown = null;
      await poll(
        "settled launcher data generation",
        async () => targetGeneration(await inspectMain(driver)),
        (identity) => {
          const generation = identity.dataGeneration;
          if (generation != null && generation === lastGeneration) {
            stableSamples += 1;
          } else {
            stableSamples = 0;
            lastGeneration = generation;
          }
          return stableSamples >= 10;
        },
        30_000,
      );
    }
    const layoutBefore = await waitForFooterLayout(driver);
    const stateBefore = asObj(await driver.getState({ timeoutMs: 5_000 }));
    const elementsBefore = asObj(await driver.getElements({ target: { type: "main" }, limit: 500 }));
    const semanticNodes = asArray(elementsBefore.elements);
    const axParity = semanticAxParity(stateBefore, layoutBefore);
    const focusGraph = semanticFocusGraph(semanticNodes);
    assert(axParity.complete, "enabled native footer semantic-to-AX parity failed");
    assert(focusGraph.reciprocal, "enabled semantic focus graph is not reciprocal");
    focusedSemanticId = focusGraph.focusedSemanticIds[0] ?? null;
    focusObservation = await observeRuntimeTaskTarget(driver, binary, { type: "main" });

    const rows = semanticNodes.filter((node) =>
      node.role === "row" && typeof node.semanticId === "string" && node.selectable !== false
    );
    const selectedBefore = asObj(stateBefore.activeListScroll ?? stateBefore.mainListScroll);
    const targetRow = [...rows].reverse().find((node) => node.kind === "builtin") ?? rows.at(-1);
    assert(targetRow && typeof targetRow.semanticId === "string", "no selectable launcher row");
    const inspectBefore = await inspectMain(driver);
    const batch = asObj(await driver.batch([{
      type: "selectBySemanticId",
      semanticId: targetRow.semanticId,
      submit: false,
    }], { timeoutMs: 8_000, stopOnError: true }));
    const batchResults = asArray(batch.results);
    assert(batchResults.length === 1 && batchResults[0].success === true, "semantic row selection failed");

    const stateAfterSelection = await poll(
      "selected row and safe scroll",
      async () => asObj(await driver.getState({ timeoutMs: 5_000 })),
      (state) => {
        const scroll = asObj(state.activeListScroll ?? state.mainListScroll);
        return typeof scroll.selectedSemanticId === "string" &&
          scroll.selectedSemanticId !== selectedBefore.selectedSemanticId &&
          scroll.selectedRowWithinSafeViewport === true;
      },
    );
    const scrollState = asObj(stateAfterSelection.activeListScroll ?? stateAfterSelection.mainListScroll);
    const inspectAfter = await inspectMain(driver);
    const afterIdentity = targetGeneration(inspectAfter);
    assert(typeof afterIdentity.dataGeneration === "number", "selected-row target data generation missing");

    const layoutAfter = await poll(
      "selected row completed-frame bounds",
      async () => asObj(await driver.getLayoutInfo({ target: { type: "main" } }, { timeoutMs: 5_000 })),
      (layout) => {
        const analysis = analyzeLayout(layout, targetReceiptFor(layout));
        const measurement = renderedSafeViewportMeasurement(
          scrollState,
          { nodes: analysis.nodes, transaction: { dataGeneration: afterIdentity.dataGeneration } },
          true,
        );
        debug = {
          selectedSemanticIdSha256: fingerprint(scrollState.selectedSemanticId),
          classification: measurement.classification,
          missingPrimitives: measurement.missingPrimitives,
          visibleRatio: measurement.visibleRatio,
          withinSafeViewport: measurement.withinSafeViewport,
          frameMatches: measurement.frameMatches,
        };
        return measurement.classification === "ok";
      },
    );
    const analysis = analyzeLayout(layoutAfter, targetReceiptFor(layoutAfter));
    const rendered = renderedSafeViewportMeasurement(
      scrollState,
      { nodes: analysis.nodes, transaction: { dataGeneration: afterIdentity.dataGeneration } },
      true,
    );
    assert(rendered.classification === "ok", "rendered selected row is not inside the safe viewport");
    assert(rendered.visibleRatio === 1 && rendered.frameMatches === true, "rendered row is clipped or stale");
    scrollObservation = await observeRuntimeTaskTarget(driver, binary, { type: "main" });
    assert(
      scrollObservation.transaction.dataGeneration === afterIdentity.dataGeneration,
      "selected-row runtime transaction changed after its completed-frame measurement",
    );
    selectedScrollState = {
      selectedSemanticId: scrollState.selectedSemanticId,
      selectedRowWithinSafeViewport: scrollState.selectedRowWithinSafeViewport,
    };

    const beforeActivation = asObj(await driver.getState({ timeoutMs: 5_000 }));
    const activationResult = asObj(await driver.triggerAction(
      "footer-action:actions",
      { host: "nativeFooter", timeoutMs: 5_000 },
    ));
    const afterActivation = await poll(
      "native Actions postcondition",
      async () => asObj(await driver.getState({ timeoutMs: 5_000 })),
      (state) => Boolean(state.actionsDialog),
    );
    const activation = nativeFooterActivationProof(
      activationResult,
      "footer-action:actions",
      !Boolean(beforeActivation.actionsDialog) && Boolean(afterActivation.actionsDialog),
    );
    assert(activation.complete, "enabled native footer activation proof failed");

    const beforeIdentity = targetGeneration(inspectBefore);
    assert(typeof beforeIdentity.windowInstanceId === "string", "main window instance identity missing");
    assert(beforeIdentity.windowInstanceId === afterIdentity.windowInstanceId, "main window instance changed during selection proof");
    assert(beforeIdentity.targetGeneration === afterIdentity.targetGeneration, "main target generation changed during selection proof");
    assert(beforeIdentity.surfaceGeneration === afterIdentity.surfaceGeneration, "main surface generation changed during selection proof");
    assert(Number(afterIdentity.dataGeneration) > Number(beforeIdentity.dataGeneration), "selection did not advance target data generation");
    proof = {
      axParity,
      focusGraph,
      activation,
      selectedRow: {
        semanticIdSha256: fingerprint(scrollState.selectedSemanticId),
        semanticIdReturnedRaw: false,
        selectionChanged: scrollState.selectedSemanticId !== selectedBefore.selectedSemanticId,
        listStateWithinSafeViewport: scrollState.selectedRowWithinSafeViewport,
        rendered: {
          classification: rendered.classification,
          rowMeasurementId: rendered.rowMeasurementId,
          safeViewportMeasurementId: rendered.safeViewportMeasurementId,
          rowObservationCount: rendered.rowObservationCount,
          safeViewportObservationCount: rendered.safeViewportObservationCount,
          rowBounds: rendered.rowBounds,
          rowVisibleBounds: rendered.rowVisibleBounds,
          rowClipBounds: rendered.rowClipBounds,
          safeViewportBounds: rendered.safeViewportBounds,
          safeViewportClipBounds: rendered.safeViewportClipBounds,
          safeViewportPaintBounds: rendered.safeViewportPaintBounds,
          coordinateSpace: rendered.coordinateSpace,
          visibleRatio: rendered.visibleRatio,
          withinSafeViewport: rendered.withinSafeViewport,
          frameGeneration: rendered.frameGeneration,
          viewportFrameGeneration: rendered.viewportFrameGeneration,
          frameMatches: rendered.frameMatches,
          targetDataGeneration: rendered.targetDataGeneration,
          missingPrimitives: rendered.missingPrimitives,
        },
        transaction: {
          before: beforeIdentity,
          after: afterIdentity,
          stableWindowInstance: beforeIdentity.windowInstanceId === afterIdentity.windowInstanceId,
          stableTargetGeneration: beforeIdentity.targetGeneration === afterIdentity.targetGeneration,
          stableSurfaceGeneration: beforeIdentity.surfaceGeneration === afterIdentity.surfaceGeneration,
          dataGenerationAdvanced: Number(afterIdentity.dataGeneration) > Number(beforeIdentity.dataGeneration),
          dataGenerationPresent: typeof afterIdentity.dataGeneration === "number",
        },
      },
    };
  } catch (error) {
    runtimeError = error instanceof Error ? error.message : "UnknownError";
    proof = { debug };
  } finally {
    if (driver) {
      try {
        await driver.close();
      } catch (error) {
        closeError = error instanceof Error ? error.name : "UnknownCloseError";
      }
    }
  }
  return {
    proof,
    runtimeError,
    cleanup: cleanupFor(driver, closeError),
    focusObservation,
    scrollObservation,
    selectedScrollState,
    focusedSemanticId,
  };
}

async function disabledScenario() {
  let driver: Driver | null = null;
  let closeError: string | null = null;
  let runtimeError: string | null = null;
  let proof: Obj = {};
  try {
    driver = await Driver.launch({
      binary,
      sessionName: `cons-proof-c06-disabled-${process.pid}`,
      sandboxHome: true,
      defaultTimeoutMs: 12_000,
      env: {
        SCRIPT_KIT_TEST_STATUS: "1",
        SCRIPT_KIT_FIDELITY_CAPTURE: "agent-chat",
        SCRIPT_KIT_TEST_FOOTER_DESCRIPTOR_FIXTURE: "disabled",
        SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
        SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
      },
    });
    driver.send({ type: "show" });
    await driver.waitForState({ windowVisible: true }, { timeoutMs: 10_000 });
    const stateBefore = await poll(
      "disabled footer descriptor",
      async () => asObj(await driver.getState({ timeoutMs: 5_000 })),
      (state) => asArray(asObj(state.activeFooter).buttons).some((button) =>
        button.id === "footer-action:actions" && button.enabled === false && typeof button.actionDisabled === "string"
      ),
    );
    const layout = await waitForFooterLayout(driver);
    const stateWithNativeFooter = asObj(await driver.getState({ timeoutMs: 5_000 }));
    const axParity = semanticAxParity(stateWithNativeFooter, layout);
    assert(axParity.complete, "disabled footer semantic-to-AX parity failed");
    const activationResult = asObj(await driver.triggerAction(
      "footer-action:actions",
      { host: "nativeFooter", timeoutMs: 5_000 },
    ));
    await Bun.sleep(200);
    const stateAfter = asObj(await driver.getState({ timeoutMs: 5_000 }));
    const unchanged = !Boolean(stateBefore.actionsDialog) && !Boolean(stateAfter.actionsDialog);
    const activation = nativeFooterActivationProof(
      activationResult,
      "footer-action:actions",
      unchanged,
      true,
    );
    assert(activation.complete, "disabled native footer activation was not refused before dispatch");
    proof = { axParity, activation, stateUnchanged: unchanged };
  } catch (error) {
    runtimeError = error instanceof Error ? error.message : "UnknownError";
  } finally {
    if (driver) {
      try {
        await driver.close();
      } catch (error) {
        closeError = error instanceof Error ? error.name : "UnknownCloseError";
      }
    }
  }
  return { proof, runtimeError, cleanup: cleanupFor(driver, closeError) };
}

const enabled = await enabledScenario();
const disabled = await disabledScenario();
const hiddenParityLayout = {
  fidelity: {
    appKit: {
      nodes: [{
        id: "fixture-button",
        accessibilityIdentifier: "footer-action:actions",
        accessibilityRole: "AXButton",
        accessibilityLabelSha256: createHash("sha256").update("Actions").digest("hex"),
        accessibilityLabelLength: 7,
        accessibilityEnabled: true,
        accessibilityElement: true,
        actionSelector: "actionsFooterAction:",
        hidden: true,
        alpha: 1,
        screenshotFrame: { x: 0, y: 0, width: 100, height: 32 },
      }],
    },
  },
};
const hiddenParity = semanticAxParity({
  activeFooter: {
    buttons: [{ id: "footer-action:actions", action: "actions", label: "Actions", enabled: true }],
  },
}, hiddenParityLayout);
const wrongHostResult = {
  host: "MainList",
  actionId: "footer-action:actions",
  ok: true,
  nativeFooterActivation: {
    semanticId: "footer-action:actions",
    accessibilityRole: "AXButton",
    actionSelector: "actionsFooterAction:",
    expectedActionSelector: "actionsFooterAction:",
    descriptorEnabled: true,
    appkitEnabled: true,
    dispatched: true,
  },
};
const onePointOverflow = renderedSafeViewportMeasurement(
  { selectedSemanticId: "row:fixture" },
  {
    transaction: { dataGeneration: 9 },
    nodes: [{
      name: "list-row:row:fixture",
      measurementId: "layout:list-row-row-fixture",
      bounds: { x: 0, y: 70, width: 100, height: 31 },
      visibleBounds: { x: 0, y: 70, width: 100, height: 31 },
      clipBounds: { x: 0, y: 70, width: 100, height: 31 },
      measurementFrameGeneration: 12,
      measurementProvenance: "paint-time",
      coordinateSpace: "window",
    }, {
      name: "main-view-main",
      measurementId: "layout:main-view-main",
      bounds: { x: 0, y: 0, width: 100, height: 100 },
      visibleBounds: { x: 0, y: 0, width: 100, height: 100 },
      clipBounds: { x: 0, y: 0, width: 100, height: 100 },
      measurementFrameGeneration: 12,
      measurementProvenance: "paint-time",
      coordinateSpace: "window",
    }],
  },
  true,
);
const missingRendered = renderedSafeViewportMeasurement(
  { selectedSemanticId: "row:fixture" },
  { transaction: { dataGeneration: 9 }, nodes: [] },
  true,
);
const negativeControls = {
  hiddenAxPeerRejected: !hiddenParity.complete && hiddenParity.peers[0]?.errors.includes("hiddenAxPeer"),
  wrongHostRejected: !nativeFooterActivationProof(
    wrongHostResult,
    "footer-action:actions",
    true,
  ).complete,
  disabledActivationRefused: asObj(disabled.proof.activation).complete === true,
  onePointBelowViewportRejected: onePointOverflow.classification === "not-ok" &&
    onePointOverflow.withinSafeViewport === false,
  missingRenderedRowBlocks: missingRendered.classification === "blocked-by-missing-primitive",
};
const runtimePassed = enabled.runtimeError == null && disabled.runtimeError == null;
const cleanupPassed = cleanupPass(enabled.cleanup) && cleanupPass(disabled.cleanup) &&
  exactExecutablePids(binary).length === 0;
const negativesPassed = Object.values(negativeControls).every(Boolean);
const axCleanup = {
  enabled: enabled.cleanup,
  disabled: disabled.cleanup,
  ownedProcessCount: exactExecutablePids(binary).length,
};
const axPrepared = runtimePassed && cleanupPassed && negativesPassed &&
    enabled.focusObservation && enabled.focusedSemanticId
  ? prepareRuntimeTaskProof("PF-007", {
      schemaVersion: 2,
      tool: "script-kit-devtools.focus",
      command: "focus.inspect",
      classification: "ok",
      proofMode: "ax",
      ...enabled.focusObservation,
      windowFocused: true,
      focusedSemanticId: enabled.focusedSemanticId,
      keyboardOwner: {
        surfaceKind: enabled.focusObservation.target.surfaceKind ?? null,
      },
      semanticProjection: {
        quality: "complete",
        proofAllowed: true,
      },
      nativeFooter: { axParity: enabled.proof.axParity },
      focusGraph: enabled.proof.focusGraph,
      activationEvidence: {
        enabled: enabled.proof.activation,
        disabled: disabled.proof.activation,
      },
      enabled: {
        axParity: enabled.proof.axParity ?? null,
        focusGraph: enabled.proof.focusGraph ?? null,
        activation: enabled.proof.activation ?? null,
      },
      disabled: disabled.proof,
      evidence: {
        accessibility: enabled.proof.axParity,
        interaction: {
          enabled: enabled.proof.activation,
          disabled: disabled.proof.activation,
        },
      },
      cleanup: axCleanup,
      missingPrimitives: [],
      errors: [],
    }, negativeControls)
  : prepareBlockedRuntimeTaskProof("PF-007", {
      stage: "native-accessibility-focus-activation",
      reason: enabled.runtimeError ?? disabled.runtimeError ?? "native runtime or cleanup did not complete",
      cleanup: axCleanup,
      controls: negativeControls,
    });
const scrollNegativeControls = {
  onePointBelowViewportRejected: negativeControls.onePointBelowViewportRejected,
  missingRenderedRowBlocks: negativeControls.missingRenderedRowBlocks,
};
const selectedRow = asObj(enabled.proof.selectedRow);
const renderedSelectedRow = asObj(selectedRow.rendered);
const scrollPrepared = runtimePassed && cleanupPassed && negativesPassed &&
    enabled.scrollObservation && selectedRow.selectionChanged === true &&
    renderedSelectedRow.classification === "ok"
  ? prepareRuntimeTaskProof("PF-008", {
      schemaVersion: 2,
      tool: "script-kit-devtools.scroll",
      command: "scroll.inspect",
      classification: "ok",
      ...enabled.scrollObservation,
      scroll: enabled.selectedScrollState,
      resizePressure: { selectedRowOutsideSafeViewport: false },
      renderedSafeViewport: {
        ...renderedSelectedRow,
        required: true,
        selectedSemanticId: enabled.selectedScrollState.selectedSemanticId,
      },
      selectedRow,
      evidence: {
        rendered: renderedSelectedRow,
        interaction: selectedRow.transaction,
      },
      cleanup: enabled.cleanup,
      missingPrimitives: [],
      errors: [],
    }, scrollNegativeControls)
  : prepareBlockedRuntimeTaskProof("PF-008", {
      stage: "rendered-safe-viewport-selection",
      reason: enabled.runtimeError ?? "selected-row runtime or cleanup did not complete",
      cleanup: enabled.cleanup,
      controls: scrollNegativeControls,
    });
const axReceipt = axPrepared.receipt;
const scrollReceipt = scrollPrepared.receipt;

await mkdir(resolve(axPath, ".."), { recursive: true });
await mkdir(resolve(scrollPath, ".."), { recursive: true });
await writeFile(axPath, `${JSON.stringify(axReceipt, null, 2)}\n`);
await writeFile(scrollPath, `${JSON.stringify(scrollReceipt, null, 2)}\n`);
console.log(JSON.stringify({ ax: axReceipt, scroll: scrollReceipt }, null, 2));

if (axPrepared.exitCode !== 0 || scrollPrepared.exitCode !== 0) {
  process.exitCode = Math.max(axPrepared.exitCode, scrollPrepared.exitCode);
}

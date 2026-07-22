import { describe, expect, test } from "bun:test";
import {
  MAIN_LIST_SCROLL_AFFORDANCE_FIELDS,
  NATIVE_LIST_CONTRACT_FIELDS,
  activeListScrollFromState,
  inspectMainListScrollAffordance,
  inspectNativeListContract,
  mainListScrollFromState,
  normalizeScriptListScrollMeasurement,
} from "./scroll.ts";
import { ProtocolCore, type Json } from "./driver.ts";

const completeAffordance = {
  atTop: true,
  atBottom: false,
  topFadeActive: false,
  topFadeProgress: 0,
  topFadeAlpha: 0,
  overscrollOffsetPx: 0,
  overscrollMaxOffsetPx: 18,
  overscrollEdge: null,
  overscrollPhase: "idle",
  generation: 4,
  lastTouchPhase: null,
  lastSettleReason: "reset",
  reducedMotion: false,
};

describe("mainListScroll affordance inspection", () => {
  test("surfaces the nested affordance snapshot unchanged", () => {
    const scroll = mainListScrollFromState({
      mainListScroll: { scrollTop: 0, affordance: completeAffordance },
    });
    const result = inspectMainListScrollAffordance(scroll, false);

    expect(result.affordance).toEqual(completeAffordance);
    expect(result.present).toBe(true);
    expect(result.complete).toBe(true);
    expect(result.missingFields).toEqual([]);
    expect(result.classification).toBe("ok");
  });

  test("keeps legacy inspection open when affordance proof is optional", () => {
    const result = inspectMainListScrollAffordance({ scrollTop: 0 }, false);

    expect(result.present).toBe(false);
    expect(result.complete).toBe(false);
    expect(result.classification).toBe("ok");
    expect(result.missingFields).toHaveLength(MAIN_LIST_SCROLL_AFFORDANCE_FIELDS.length);
  });

  test("fails closed when required affordance proof is absent", () => {
    const result = inspectMainListScrollAffordance({ scrollTop: 0 }, true);

    expect(result.classification).toBe("blocked-by-missing-primitive");
    expect(result.missingFields).toContain("mainListScroll.affordance.atTop");
    expect(result.missingFields).toContain("mainListScroll.affordance.lastSettleReason");
  });

  test("names every missing field from a partial affordance snapshot", () => {
    const result = inspectMainListScrollAffordance(
      { affordance: { atTop: true, overscrollPhase: "idle" } },
      true,
    );

    expect(result.classification).toBe("blocked-by-missing-primitive");
    expect(result.missingFields).not.toContain("mainListScroll.affordance.atTop");
    expect(result.missingFields).not.toContain("mainListScroll.affordance.overscrollPhase");
    expect(result.missingFields).toContain("mainListScroll.affordance.atBottom");
    expect(result.missingFields).toHaveLength(MAIN_LIST_SCROLL_AFFORDANCE_FIELDS.length - 2);
  });

  test("treats present nullable fields as complete protocol fields", () => {
    const result = inspectMainListScrollAffordance(
      { affordance: { ...completeAffordance, overscrollEdge: null, lastTouchPhase: null } },
      true,
    );

    expect(result.complete).toBe(true);
    expect(result.missingFields).toEqual([]);
    expect(result.classification).toBe("ok");
  });
});

describe("activeListScroll native contract inspection", () => {
  const complete = Object.fromEntries(NATIVE_LIST_CONTRACT_FIELDS.map((field) => [field, null]));

  test("prefers the active surface and falls back to Script List", () => {
    expect(activeListScrollFromState({ activeListScroll: { surface: "tips" }, mainListScroll: { surface: "script_list" } }).surface).toBe("tips");
    expect(activeListScrollFromState({ mainListScroll: { surface: "script_list" } }).surface).toBe("script_list");
  });

  test("requires field presence while allowing nullable empty/offscreen values", () => {
    const result = inspectNativeListContract(complete, true);
    expect(result.complete).toBe(true);
    expect(result.classification).toBe("ok");
  });

  test("fails closed with exact missing semantic and viewport fields", () => {
    const result = inspectNativeListContract({ surface: "tips" }, true);
    expect(result.classification).toBe("blocked-by-missing-primitive");
    expect(result.missingFields).toContain("activeListScroll.selectedSemanticId");
    expect(result.missingFields).toContain("activeListScroll.scrollTopOffsetPx");
  });

  test("does not require ScriptList layout bounds for a built-in contract", () => {
    const scroll = { ...complete, surface: "tips", viewportHeight: 0, scrollTop: 0 };
    const result = inspectNativeListContract(scroll, true);
    expect(result.classification).toBe("ok");
  });
});

class CapturingProtocol extends ProtocolCore {
  writes: Json[] = [];

  constructor() {
    super(500, "scroll-test");
  }

  protected writeCommand(payload: Json): void {
    this.writes.push(payload);
    queueMicrotask(() => {
      if (payload.type === "getState") {
        this.handleResponse({
          type: "stateResult",
          requestId: payload.requestId,
          activeListScroll: { surface: "tips", implementation: "uniform_list" },
        });
        return;
      }
      this.handleResponse({
        type: "simulateGpuiEventResult",
        requestId: payload.requestId,
        success: true,
      });
    });
  }

  get alive(): boolean {
    return true;
  }

  async close(): Promise<void> {}
}

test("typed scroll-wheel helper emits the exact pixel-only phased wire event", async () => {
  const protocol = new CapturingProtocol();
  await protocol.simulateGpuiScrollWheel(
    { x: 12.5, y: 48, deltaX: 0, deltaY: 36, phase: "moved" },
    { target: { type: "main" } },
  );

  const command = protocol.writes[0];
  expect(command.type).toBe("simulateGpuiEvent");
  expect(command.target).toEqual({ type: "main" });
  expect(command.event).toEqual({
    type: "scrollWheel",
    x: 12.5,
    y: 48,
    deltaX: 0,
    deltaY: 36,
    phase: "moved",
  });
  expect(command.event.deltaMode).toBeUndefined();
});

test("typed scroll-wheel helper preserves direct and momentum lifecycle fields", async () => {
  const protocol = new CapturingProtocol();
  const phases = ["began", "changed", "ended"] as const;
  for (const [index, phase] of phases.entries()) {
    await protocol.simulateGpuiScrollWheel({
      x: 10,
      y: 20,
      deltaX: 0,
      deltaY: index === 1 ? 3.25 : 0,
      phase: index === 0 ? "started" : index === 2 ? "ended" : "moved",
      directPhase: phase,
      momentumPhase: phase,
      timestampSeconds: 100.5 + index,
    });
  }
  expect(protocol.writes.map((command) => command.event.directPhase)).toEqual(phases);
  expect(protocol.writes.map((command) => command.event.momentumPhase)).toEqual(phases);
  expect(protocol.writes[1].event.deltaY).toBe(3.25);
  expect(protocol.writes[2].event.timestampSeconds).toBe(102.5);
});

test("typed active-list helper reads the canonical state field", async () => {
  const protocol = new CapturingProtocol();
  const receipt = await protocol.getActiveListScroll();
  expect(receipt.surface).toBe("tips");
  expect(receipt.implementation).toBe("uniform_list");
});

test("an intentionally offscreen selected row keeps viewport measurement valid", () => {
  const normalized = normalizeScriptListScrollMeasurement(
    {
      scrollTop: 240,
      viewportHeight: 0,
      safeViewportHeight: 0,
      selectedIndex: 1,
      selectedSemanticId: "main-list-row:script/example",
      selectedStableKey: "script/example",
      selectedRowVisible: false,
      selectedRowAboveFooter: false,
      selectedRowWithinSafeViewport: false,
    },
    {
      nodes: [
        { name: "ScriptList", bounds: { x: 0, y: 60, width: 640, height: 360 } },
        { name: "MainViewFooter", bounds: { x: 0, y: 390, width: 640, height: 30 } },
      ],
    },
  );

  expect(normalized.classification).toBeNull();
  expect(normalized.missingPrimitive).toBeNull();
  expect(normalized.effectiveViewportHeight).toBe(360);
  expect(normalized.effectiveSafeViewportHeight).toBe(330);
  expect(normalized.scroll.selectedRowTop).toBeNull();
  expect(normalized.scroll.selectedRowBottom).toBeNull();
  expect(normalized.scroll.selectedRowVisible).toBe(false);
  expect(normalized.viewportMeasurementWarning).toContain("selectedRowBoundsUnavailable");
});

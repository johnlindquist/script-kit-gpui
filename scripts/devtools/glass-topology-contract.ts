export type NativeWindowRow = {
  windowId?: number;
  ownerPid?: number;
  title?: string;
  layer?: number;
  alpha?: number;
  onscreen?: boolean;
  bounds?: { x?: number; y?: number; width?: number; height?: number };
};

export type NativeWindowClass =
  | "Main"
  | "Actions"
  | "Notes"
  | "Dictation"
  | "permittedAuxiliary"
  | "footerChild"
  | "unknownStale";

export function classifyNativeWindow(
  window: NativeWindowRow,
  mainWindowId: number,
): NativeWindowClass {
  const id = Number(window.windowId);
  const title = String(window.title ?? "");
  const width = Number(window.bounds?.width ?? 0);
  const height = Number(window.bounds?.height ?? 0);
  if (id === mainWindowId) return "Main";
  if (/actions|command palette/i.test(title)) return "Actions";
  if (title === "Notes") return "Notes";
  if (title === "Script Kit Dictation") return "Dictation";
  if (title === "" && Number(window.layer) === 101 && height > 140) {
    return "Actions";
  }
  if (
    title === ""
    && window.onscreen === false
    && Number(window.layer) === 0
    && width === 500
    && height === 500
  ) {
    return "permittedAuxiliary";
  }
  if (title === "" && width > 40 && height > 0 && height <= 140) {
    return "footerChild";
  }
  return "unknownStale";
}

export function classifyNativeInventory(
  windows: NativeWindowRow[],
  pid: number,
  mainWindowId: number,
) {
  const rows = windows.map((window) => ({
    ...window,
    classification: classifyNativeWindow(window, mainWindowId),
  }));
  const errors: string[] = [];
  if (rows.some((row) => Number(row.ownerPid) !== pid)) {
    errors.push("same-PID inventory contains a mismatched owner PID");
  }
  for (const classification of ["Main", "Actions", "Notes", "Dictation"] as const) {
    const matching = rows.filter((row) => row.classification === classification);
    if (matching.length > 1) {
      errors.push(`${classification} has ${matching.length} complete native owners`);
    }
  }
  if (rows.some((row) => row.classification === "footerChild")) {
    errors.push("detached footer child window present");
  }
  if (rows.some((row) => row.classification === "unknownStale")) {
    errors.push("unknown or stale same-PID native window present");
  }
  return {
    schemaVersion: 1,
    includesHiddenOffscreenAndAlphaZero: true,
    rows,
    errors,
    pass: errors.length === 0,
  };
}

export function deriveUniqueOwnerDelta(
  before: NativeWindowRow[],
  after: NativeWindowRow[],
  classification: NativeWindowClass,
  pid: number,
  mainWindowId: number,
) {
  const prior = new Set(before.map((window) => Number(window.windowId)));
  const candidates = classifyNativeInventory(after, pid, mainWindowId).rows
    .filter((window) =>
      window.classification === classification
      && !prior.has(Number(window.windowId))
    );
  return {
    classification,
    candidateIds: candidates.map((window) => Number(window.windowId)),
    pass: candidates.length === 1,
  };
}

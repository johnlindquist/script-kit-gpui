export type FilmstripIdentity = {
  runId: string;
  gitCommit: string;
  binarySha256: string;
  pid: number;
  windowId: number;
};

export function validateFilmstripCapture(
  receipt: any,
  expected: FilmstripIdentity,
): string[] {
  const errors: string[] = [];
  if (!receipt || typeof receipt !== "object") return ["filmstrip receipt missing"];
  if (receipt.schemaVersion !== 2) errors.push("filmstrip schemaVersion must be 2");
  if (receipt.status !== "ok") errors.push("filmstrip status must be ok");
  if (receipt.captureHealthPass !== true) {
    errors.push("filmstrip captureHealthPass must be true");
  }
  if (receipt.runID !== expected.runId) errors.push("filmstrip runId mismatch");
  if (receipt.gitCommit !== expected.gitCommit) errors.push("filmstrip gitCommit mismatch");
  if (receipt.binarySHA256 !== expected.binarySha256) {
    errors.push("filmstrip binary SHA-256 mismatch");
  }
  if (Number(receipt.pid) !== expected.pid) errors.push("filmstrip PID mismatch");
  if (Number(receipt.windowID) !== expected.windowId) {
    errors.push("filmstrip expected CGWindowID mismatch");
  }
  if (!Number.isFinite(Number(receipt.displayID))) errors.push("display ID missing");
  if (!(Number(receipt.refreshRateHz) > 0)) errors.push("refresh rate missing");
  if (!(Number(receipt.captureScale) > 0)) errors.push("capture scale missing");
  if (receipt.pixelFormat !== "BGRA") errors.push("pixel format must be BGRA");

  const received = Number(receipt.receivedSampleCount);
  const accounted = Number(receipt.accountedSampleCount);
  const complete = Number(receipt.completeSampleCount);
  const copied = Number(receipt.copiedCompleteCount);
  const encoded = Number(receipt.encodedCompleteCount);
  const incomplete = Number(receipt.incompleteSampleCount);
  const missingDisplayTime = Number(receipt.missingDisplayTimeCount);
  const dropped = Number(receipt.droppedCompleteCount);
  const duplicates = Number(receipt.duplicateDisplayTimeCount);
  const late = Number(receipt.lateFrameCount);
  if (![received, accounted, complete, copied, encoded, incomplete,
    missingDisplayTime, dropped, duplicates, late]
    .every(Number.isFinite)) {
    errors.push("capture accounting fields missing");
  } else {
    if (received !== accounted) errors.push("received sample accounting mismatch");
    if (accounted !== complete + incomplete) {
      errors.push("complete plus incomplete sample accounting mismatch");
    }
    if (missingDisplayTime !== 0) {
      errors.push("one or more samples lack display time");
    }
    if (complete !== copied) errors.push("copied complete count mismatch");
    if (copied !== encoded) errors.push("encoded complete count mismatch");
    if (dropped !== 0) errors.push("dropped complete count must be zero");
    if (duplicates !== 0) errors.push("duplicate display time observed");
    if (late !== 0) errors.push("display-time coverage gap observed");
  }
  if (
    Number(receipt.maximumConsecutiveDisplayTimeGapNs)
      > Number(receipt.maximumAllowedDisplayTimeGapNs)
  ) {
    errors.push("maximum display-time gap exceeds one period plus 1ms");
  }
  const frames = Array.isArray(receipt.frames) ? receipt.frames : [];
  if (frames.length !== encoded) errors.push("encoded frame inventory mismatch");
  const hasOwnedFrame = frames.some(
    (frame: any) => Number(frame?.actualWindowID) === expected.windowId,
  );
  for (const [index, frame] of frames.entries()) {
    if (Number(frame?.expectedWindowID) !== expected.windowId) {
      errors.push(`frame ${index} expected CGWindowID mismatch`);
    }
    const absentPinnedWindow = hasOwnedFrame
      && frame?.actualWindowID == null
      && frame?.windowBounds == null;
    if (
      Number(frame?.actualWindowID) !== expected.windowId
      && !absentPinnedWindow
    ) {
      errors.push(`frame ${index} actual CGWindowID mismatch`);
    }
    if (!(Number(frame?.displayTimeNs) > 0)) {
      errors.push(`frame ${index} host display time missing`);
    }
    if (!/^[a-f0-9]{64}$/.test(String(frame?.sha256 ?? ""))) {
      errors.push(`frame ${index} SHA-256 missing`);
    }
  }
  return errors;
}

export function validateDetachedExitLifecycle(
  receipt: any,
  expectedWindowId: number,
  expectedState: "exiting" | "cancelled",
): string[] {
  const errors: string[] = [];
  if (receipt?.schemaVersion !== 2) errors.push("native exit schemaVersion must be 2");
  if (Number(receipt?.nativeWindowNumber) !== expectedWindowId) {
    errors.push("native exit CGWindowID mismatch");
  }
  if (receipt?.exitMode !== "DetachedRegionsFadeOnly") {
    errors.push("native exit mode must be DetachedRegionsFadeOnly");
  }
  if (expectedState === "exiting") {
    const original = receipt?.originalFrame;
    const current = receipt?.currentFrame;
    if (
      !Array.isArray(original)
      || !Array.isArray(current)
      || original.length !== 4
      || current.length !== 4
      || original.some((value: number, index: number) =>
        Math.abs(value - Number(current[index])) > 0.25
      )
    ) {
      errors.push("native exit frame moved by more than 0.5 device pixel");
    }
  }
  if (Number(receipt?.commonContentViewFilterCount) !== 0) {
    errors.push("common content-view filter must remain absent");
  }
  if (receipt?.glassHostAttached !== true) {
    errors.push("native glass host detached before current exit resolved");
  }
  const request = Number(receipt?.requestHostTimeNs);
  const deadline = Number(receipt?.expectedRemovalDeadlineNs);
  if (!Number.isFinite(request) || deadline - request !== 135_000_000) {
    errors.push("native exit removal deadline is not exactly 135ms");
  }
  const events = Array.isArray(receipt?.history) ? receipt.history : [];
  if (!events.some((event: any) => event?.event === "ticketBegin")) {
    errors.push("native exit ticket-begin event missing");
  }
  if (expectedState === "exiting") {
    if (receipt?.cancelledAtHostTimeNs != null) {
      errors.push("active native exit was already cancelled");
    }
    if (receipt?.committedAtHostTimeNs != null) {
      errors.push("active native exit committed before deadline");
    }
  } else {
    if (!Number.isFinite(Number(receipt?.cancelledAtHostTimeNs))) {
      errors.push("reopened native exit lacks cancellation time");
    }
    if (!events.some((event: any) => event?.event === "ticketCancel")) {
      errors.push("native exit ticket-cancel event missing");
    }
    if (Number(receipt?.currentAlpha) < 0.999) {
      errors.push("cancelled native exit did not restore alpha");
    }
    if (receipt?.committedAtHostTimeNs != null) {
      errors.push("cancelled native exit was incorrectly committed");
    }
  }
  return errors;
}

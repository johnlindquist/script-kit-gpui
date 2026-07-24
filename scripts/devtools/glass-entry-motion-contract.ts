export type NativeWindowBounds =
  | [[number, number], [number, number]]
  | null
  | undefined;

type EntryFrame = {
  sequence?: number;
  windowAlpha?: number | null;
  windowBounds?: NativeWindowBounds;
};

function size(bounds: NativeWindowBounds) {
  if (
    !Array.isArray(bounds)
    || !Array.isArray(bounds[1])
    || !Number.isFinite(Number(bounds[1][0]))
    || !Number.isFinite(Number(bounds[1][1]))
  ) {
    return null;
  }
  return {
    width: Number(bounds[1][0]),
    height: Number(bounds[1][1]),
  };
}

export function analyzeEntryMotionEnvelope(
  frames: EntryFrame[],
  settledBounds: NativeWindowBounds,
  expectedFirstWidthScale: number,
) {
  const settled = size(settledBounds);
  const errors: string[] = [];
  if (!settled || settled.width <= 0 || settled.height <= 0) {
    return {
      pass: false,
      errors: ["settled native bounds are missing"],
      frames: [],
    };
  }

  const measured = frames.flatMap((frame) => {
    const measuredSize = size(frame.windowBounds);
    const alpha = Number(frame.windowAlpha);
    if (!measuredSize || !Number.isFinite(alpha) || alpha < 0.001) return [];
    return [{
      sequence: frame.sequence ?? null,
      alpha,
      widthScale: measuredSize.width / settled.width,
      heightScale: measuredSize.height / settled.height,
    }];
  });
  const first = measured[0];
  const distinctWidths = new Set(
    measured.map((frame) => frame.widthScale.toFixed(4)),
  ).size;
  if (measured.length < 3) errors.push(`only ${measured.length}/3 visible geometry frames`);
  if (distinctWidths < 2) errors.push("entry filmstrip did not capture geometry motion");
  if (
    first == null
    || Math.abs(first.widthScale - expectedFirstWidthScale) > 0.025
  ) {
    errors.push(
      `first visible width scale must remain ${expectedFirstWidthScale.toFixed(2)} ±0.025`,
    );
  }
  if (
    measured.some((frame) =>
      frame.widthScale < 0.925 || frame.widthScale > 1.075
    )
  ) {
    errors.push("visible width escaped the locked 92.5%-107.5% envelope");
  }
  if (
    measured.some((frame) =>
      frame.heightScale < 0.965 || frame.heightScale > 1.035
    )
  ) {
    errors.push("visible height escaped the locked 96.5%-103.5% envelope");
  }

  return {
    expectedFirstWidthScale,
    settled,
    firstVisible: first ?? null,
    minimumWidthScale: measured.length
      ? Math.min(...measured.map((frame) => frame.widthScale))
      : null,
    maximumWidthScale: measured.length
      ? Math.max(...measured.map((frame) => frame.widthScale))
      : null,
    minimumHeightScale: measured.length
      ? Math.min(...measured.map((frame) => frame.heightScale))
      : null,
    maximumHeightScale: measured.length
      ? Math.max(...measured.map((frame) => frame.heightScale))
      : null,
    frames: measured,
    errors,
    pass: errors.length === 0,
  };
}

export function analyzeLoggedEntryGeometry(
  logLines: string[],
  expectedFirstWidthScale: number,
) {
  const line = [...logLines].reverse().find((candidate) =>
    candidate.includes("variant=window_frame")
    && candidate.includes("phase=enter")
  );
  const match = line?.match(
    /frames=(\d+)x(\d+)->(\d+)x(\d+)->(\d+)x(\d+)/,
  );
  if (!match) {
    return {
      pass: false,
      errors: ["runtime entry geometry log is missing"],
      line: line ?? null,
    };
  }
  const [
    startWidth,
    startHeight,
    squishWidth,
    squishHeight,
    finalWidth,
    finalHeight,
  ] = match.slice(1).map(Number);
  const firstWidthScale = startWidth / finalWidth;
  const firstHeightScale = startHeight / finalHeight;
  const squishWidthScale = squishWidth / finalWidth;
  const squishHeightScale = squishHeight / finalHeight;
  const errors: string[] = [];
  if (Math.abs(firstWidthScale - expectedFirstWidthScale) > 0.025) {
    errors.push(
      `logged start width scale must remain ${expectedFirstWidthScale.toFixed(2)} ±0.025`,
    );
  }
  if (
    [firstWidthScale, squishWidthScale].some(
      (scale) => scale < 0.925 || scale > 1.075,
    )
  ) {
    errors.push("logged width escaped the locked 92.5%-107.5% envelope");
  }
  if (
    [firstHeightScale, squishHeightScale].some(
      (scale) => scale < 0.965 || scale > 1.035,
    )
  ) {
    errors.push("logged height escaped the locked 96.5%-103.5% envelope");
  }
  return {
    line,
    expectedFirstWidthScale,
    start: { width: startWidth, height: startHeight },
    squish: { width: squishWidth, height: squishHeight },
    final: { width: finalWidth, height: finalHeight },
    firstWidthScale,
    firstHeightScale,
    squishWidthScale,
    squishHeightScale,
    errors,
    pass: errors.length === 0,
  };
}

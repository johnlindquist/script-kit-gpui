/** One fail-closed rectangle contract shared by layout, text, and receipt proof. */
export type EvidenceRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export function isValidEvidenceRect(value: unknown, allowEmpty = false): value is EvidenceRect {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const rect = value as Record<string, unknown>;
  if (
    !["x", "y", "width", "height"].every((field) =>
      typeof rect[field] === "number" && Number.isFinite(rect[field])
    )
  ) return false;
  return allowEmpty
    ? Number(rect.width) >= 0 && Number(rect.height) >= 0
    : Number(rect.width) > 0 && Number(rect.height) > 0;
}

export function evidenceIntersectionRatio(bounds: EvidenceRect, visible: EvidenceRect): number {
  if (!isValidEvidenceRect(bounds) || !isValidEvidenceRect(visible, true)) return 0;
  const width = Math.max(
    0,
    Math.min(bounds.x + bounds.width, visible.x + visible.width) -
      Math.max(bounds.x, visible.x),
  );
  const height = Math.max(
    0,
    Math.min(bounds.y + bounds.height, visible.y + visible.height) -
      Math.max(bounds.y, visible.y),
  );
  return (width * height) / (bounds.width * bounds.height);
}

import { expect, test } from "bun:test";
import { validateReliabilityState } from "./ai_reliability_cli.ts";

test("Quick AI red receipt recognizes the search-budget defect safely", () => {
  const assertions = validateReliabilityState(
    {
      schemaVersion: 1,
      surface: "quickAi",
      primaryActionId: null,
      recoveryActions: [],
      diagnostic: { redacted: true, rawPrimaryVisible: true },
    },
    "quickAi",
  );
  expect(Object.values(assertions).every(Boolean)).toBe(true);
});

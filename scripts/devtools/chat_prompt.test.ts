import { expect, test } from "bun:test";
import { validateReliabilityState } from "./ai_reliability_cli.ts";

test("ChatPrompt red receipt recognizes the client-too-old defect safely", () => {
  const assertions = validateReliabilityState(
    {
      schemaVersion: 1,
      surface: "chatPrompt",
      primaryActionId: null,
      recoveryActions: [],
      diagnostic: { redacted: true, rawPrimaryVisible: true },
    },
    "chatPrompt",
  );
  expect(Object.values(assertions).every(Boolean)).toBe(true);
});

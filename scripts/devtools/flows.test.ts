import { expect, test } from "bun:test";
import { validateReliabilityState } from "./ai_reliability_cli.ts";

test("Flow red receipt recognizes the protocol defect safely", () => {
  const assertions = validateReliabilityState(
    {
      schemaVersion: 1,
      surface: "flowConversation",
      primaryActionId: null,
      recoveryActions: [],
      diagnostic: { redacted: true, rawPrimaryVisible: true },
    },
    "flowConversation",
  );
  expect(Object.values(assertions).every(Boolean)).toBe(true);
});

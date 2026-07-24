import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { Driver } from "./driver.ts";

type JsonObject = Record<string, unknown>;

export function validateReliabilityState(
  state: JsonObject,
  expectedSurface: string,
) {
  const diagnostic = state.diagnostic as JsonObject | undefined;
  const recoveryActions = state.recoveryActions;
  return {
    surfaceMatches: state.surface === expectedSurface,
    schemaMatches: state.schemaVersion === 1,
    rawPrimaryVisible: diagnostic?.rawPrimaryVisible === true,
    redactedDiagnostic: diagnostic?.redacted === true,
    noRecoveryAction:
      state.primaryActionId == null &&
      Array.isArray(recoveryActions) &&
      recoveryActions.length === 0,
    noRawPayloadField: !Object.keys(state).some((key) =>
      /raw(payload|detail|error)/i.test(key)
    ),
  };
}

function artifactBinary() {
  const artifact = resolve(
    "target-agent/artifacts/ai-rock-solid-ux-red/script-kit-gpui",
  );
  return existsSync(artifact) ? artifact : undefined;
}

export async function inspectAiReliabilityFixture(
  tool: string,
  fixtureId: string,
  expectedSurface: string,
  strict: boolean,
) {
  const target = { type: "main" };
  const driver = await Driver.launch({
    sessionName: `ai-reliability-${expectedSurface}`,
    sandboxHome: true,
    binary: artifactBinary(),
  });
  try {
    const windows = await driver.listAutomationWindows();
    const windowList = (windows.windows ?? []) as JsonObject[];
    const resolvedTarget =
      windowList.find((window) => window.kind === "main") ?? null;
    const fixture = await driver.request(
      { type: "setAiReliabilityTestFixture", fixtureId, target },
      { expect: "aiReliabilityTestFixtureResult" },
    );
    const state = await driver.request(
      { type: "getAiReliabilityState", target },
      { expect: "aiReliabilityStateResult" },
    );
    const elements = await driver.getElements({ target, limit: 200 });
    const layout = await driver.getLayoutInfo({ target });
    const surfaceMatches = state.surface === expectedSurface;
    const sameTarget =
      resolvedTarget !== null &&
      (resolvedTarget as JsonObject).id === "main";
    const fixtureInstalled = fixture.success === true;
    const primitiveStack = [
      { name: "getAiReliabilityState", ok: state.schemaVersion === 1 },
      { name: "getElements", ok: elements.type === "elementsResult" },
      { name: "getLayoutInfo", ok: layout.type === "layoutInfoResult" },
    ];
    const stateAssertions = validateReliabilityState(state, expectedSurface);
    const strictPass =
      fixtureInstalled &&
      surfaceMatches &&
      sameTarget &&
      primitiveStack.every((primitive) => primitive.ok);
    const receipt = {
      schemaVersion: 1,
      tool,
      command: "inspect",
      classification: strictPass ? "reproduced" : "blocked-by-missing-primitive",
      strict,
      fixtureId,
      requestedTarget: target,
      resolvedTarget,
      sameTarget,
      state,
      primitiveStack,
      assertions: {
        fixtureInstalled,
        ...stateAssertions,
      },
      redaction: {
        rawProviderPayloadStored: false,
        userTextStored: false,
        pathStored: false,
        fingerprintOnly: true,
      },
      errors: strict && !strictPass ? ["strict AI reliability receipt failed"] : [],
    };
    console.log(JSON.stringify(receipt, null, 2));
    if (strict && !strictPass) {
      process.exitCode = 1;
    }
  } finally {
    await driver.close();
  }
}

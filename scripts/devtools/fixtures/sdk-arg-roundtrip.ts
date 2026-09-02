// This is the sole SDK entrypoint registered by sdk-journey.ts/runtime_sdk.rs.
// It uses the production SDK, not SDK_TEST_AUTOSUBMIT or a local prompt resolver.
import "../../kit-sdk.ts";
import { writeSync } from "node:fs";

const fixtureId = "sdk.arg-roundtrip.v1";
if (process.env.SDK_TEST_AUTOSUBMIT !== "0" || process.env.SCRIPT_KIT_SDK_FIXTURE !== fixtureId) {
  throw new Error("reviewed_sdk_fixture_environment_required");
}
let input = "";
const received: Array<{ type: string; id: string; value: string | null }> = [];
let submissionCount = 0;
let submittedValue: string | null = null;
let rpcResolved = false;
let promptResolved = false;
const report = (event: string, extra: Record<string, unknown> = {}) => {
  writeSync(2, `SDK_FIXTURE ${JSON.stringify({ fixtureId, event, submissionCount, submittedValue,
    rpcResolved, promptResolved, received, ...extra })}\n`);
};
// Observation only. The SDK's existing stdin listener is the sole resolver and
// cancellation owner; prepend observes its null input before process.exit(0).
process.stdin.prependListener("data", (chunk: string | Buffer) => {
  input += chunk.toString();
  if (input.length > 65536) throw new Error("sdk_fixture_input_limit");
  let newline: number;
  while ((newline = input.indexOf("\n")) >= 0) {
    const message = JSON.parse(input.slice(0, newline));
    input = input.slice(newline + 1);
    if (message.type === "submit") {
      received.push({ type: message.type, id: message.id, value: message.value });
      if (received.length > 4) throw new Error("sdk_fixture_completion_limit");
    }
  }
});
process.on("exit", code => report("exit", { code }));
const prompt = arg({ placeholder: "Owned SDK completion",
  choices: [{ name: "SDK first", value: "sdk-first" }, { name: "SDK second", value: "sdk-second" }],
  onSubmit: async value => { submissionCount++; submittedValue = value; },
});
const state = await getState();
rpcResolved = true;
report("rpc", { state });
const value = await prompt;
promptResolved = true;
report("resolved", { value });

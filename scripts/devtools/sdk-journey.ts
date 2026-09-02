import { createHash } from "node:crypto";
import { lstatSync, mkdirSync, readFileSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnOwnedProcess, type OwnedProcess } from "../agentic/owned-process.ts";
import { createOwnedStagingDirectory, type OutputClaim, type OwnedCleanup } from "../agentic/artifact-lifecycle.ts";
import type { ArtifactReference } from "../agentic/build-artifact.ts";
import { boundedObservation, DriverCommandRefused, DriverLifecycleError, unknownOwnedCleanup, type Json } from "./driver.ts";
import { OwnedEvaluationClient, EvaluationContractError, type SdkPromptCommand, type SdkPromptResult } from "./lib/owned-evaluation.ts";
import type { AutomationInstance } from "./lib/target-identity.ts";
import { aggregateCleanup } from "./lib/story-contract.ts";
import type { RuntimeJourneyReceipt } from "./design.ts";

export const SDK_FIXTURE_ID = "sdk.arg-roundtrip.v1" as const;
const ROOT = resolve(import.meta.dir, "../..");
const ENTRYPOINT = join(ROOT, "scripts/devtools/fixtures/sdk-arg-roundtrip.ts");
const SDK = join(ROOT, "scripts/kit-sdk.ts");
type Case = "submit" | "cancel" | "full" | "disconnected";
interface SdkFixtureEvidence {
  fixtureId: string; event: string; submissionCount: number; submittedValue: string | null;
  rpcResolved: boolean; promptResolved: boolean;
  received: Array<{ type: string; id: string; value: string | null }>;
  code?: number; state?: Json; value?: string;
}
interface SdkChild {
  process: OwnedProcess;
  prompt: Promise<Json>;
  rpc: Promise<Json>;
  rpcEvidence: Promise<SdkFixtureEvidence>;
  exitEvidence: Promise<SdkFixtureEvidence>;
  readers: Promise<void>;
  provenance: Json;
}

async function required<T>(promise: Promise<T>): Promise<T> {
  const observed = await boundedObservation(promise, 8000);
  if (observed.completed === false) throw new EvaluationContractError("sdk_observation_failed", [String(observed.error)]);
  return observed.value;
}

/** No caller-provided argv, preload, environment, script, or command is accepted. */
async function launchSdkChild(claim: OutputClaim, caseId: Case): Promise<SdkChild> {
  const hashes: Record<string, string> = {};
  for (const [name, path] of [["entrypointSha256", ENTRYPOINT], ["sdkSha256", SDK]]) {
    const stat = lstatSync(path!);
    if (!stat.isFile() || stat.isSymbolicLink() || realpathSync(path!) !== path)
      throw new EvaluationContractError("sdk_fixture_source_not_regular");
    hashes[name!] = createHash("sha256").update(readFileSync(path!)).digest("hex");
  }
  const sandbox = createOwnedStagingDirectory(claim, { name: `sdk-${caseId}` });
  const home = join(sandbox, "home"), tmp = join(sandbox, "tmp");
  for (const path of [home, tmp, join(home, ".scriptkit"), join(home, ".config"), join(home, ".cache")])
    mkdirSync(path, { recursive: true, mode: 0o700 });
  const child = await spawnOwnedProcess({ argv: [process.execPath, ENTRYPOINT], cwd: sandbox,
    env: { HOME: home, SK_PATH: join(home, ".scriptkit"), CODEX_HOME: join(home, ".codex"),
      XDG_CONFIG_HOME: join(home, ".config"), XDG_DATA_HOME: join(home, ".local/share"),
      XDG_CACHE_HOME: join(home, ".cache"), TMPDIR: tmp, LANG: "en_US.UTF-8", TZ: "UTC",
      PATH: `${dirname(process.execPath)}:/opt/homebrew/bin:/usr/bin:/bin`,
      SDK_TEST_AUTOSUBMIT: "0", SCRIPT_KIT_SDK_FIXTURE: SDK_FIXTURE_ID },
    timeoutMs: 45000, maxOutputBytes: 1024 * 1024 });
  const prompt = Promise.withResolvers<Json>(), rpc = Promise.withResolvers<Json>();
  const rpcEvidence = Promise.withResolvers<SdkFixtureEvidence>(), exitEvidence = Promise.withResolvers<SdkFixtureEvidence>();
  const pending = [prompt, rpc, rpcEvidence, exitEvidence];
  // A failure can precede the particular observer; retain it without an unhandled rejection.
  for (const item of pending) void item.promise.catch(() => {});
  let promptCount = 0, rpcCount = 0, rpcEvidenceCount = 0, exitEvidenceCount = 0;
  const readLines = async (stream: ReadableStream<Uint8Array>, consume: (line: string) => void) => {
    let buffer = "";
    const decoder = new TextDecoder();
    for await (const chunk of stream) {
      buffer += decoder.decode(chunk, { stream: true });
      if (buffer.length > 65536) throw new EvaluationContractError("sdk_fixture_line_limit");
      let newline: number;
      while ((newline = buffer.indexOf("\n")) >= 0) {
        const line = buffer.slice(0, newline); buffer = buffer.slice(newline + 1);
        if (line) consume(line);
      }
    }
    buffer += decoder.decode();
    if (buffer) throw new EvaluationContractError("sdk_fixture_truncated_jsonl");
  };
  const consumers = [
    readLines(child.stdout, line => {
      const message = JSON.parse(line) as Json;
      if (message.type === "arg" && ++promptCount === 1) prompt.resolve(message);
      else if (message.type === "getState" && message.requestId === "2" && Object.keys(message).sort().join(",") === "requestId,type" && ++rpcCount === 1) rpc.resolve(message);
      else throw new EvaluationContractError("sdk_fixture_unregistered_message");
    }),
    readLines(child.stderr, line => {
      if (!line.startsWith("SDK_FIXTURE ")) return;
      const evidence = JSON.parse(line.slice("SDK_FIXTURE ".length)) as SdkFixtureEvidence;
      if (evidence.fixtureId !== SDK_FIXTURE_ID) throw new EvaluationContractError("sdk_fixture_identity_mismatch");
      if (evidence.event === "rpc" && ++rpcEvidenceCount === 1) rpcEvidence.resolve(evidence);
      else if (evidence.event === "exit" && ++exitEvidenceCount === 1) exitEvidence.resolve(evidence);
      else if (evidence.event !== "resolved") throw new EvaluationContractError("sdk_fixture_unknown_observation");
    }),
  ];
  for (const consumer of consumers) void consumer.catch(error => { for (const item of pending) item.reject(error); });
  const readers = Promise.allSettled(consumers).then(results => {
    for (const result of results) if (result.status === "rejected") throw result.reason;
    if (promptCount !== 1 || rpcCount !== 1) throw new EvaluationContractError("sdk_fixture_request_missing");
  });
  void readers.catch(error => { for (const item of pending) item.reject(error); });
  return { process: child, prompt: prompt.promise, rpc: rpc.promise, rpcEvidence: rpcEvidence.promise,
    exitEvidence: exitEvidence.promise, readers,
    provenance: { fixtureId: SDK_FIXTURE_ID, entrypoint: "scripts/devtools/fixtures/sdk-arg-roundtrip.ts",
      sdk: "scripts/kit-sdk.ts", ...hashes, identity: child.identity, sandbox } };
}

async function control(client: OwnedEvaluationClient, target: AutomationInstance, command: SdkPromptCommand): Promise<SdkPromptResult> {
  const state = await client.inspect(target);
  const result = await client.design({ operation: "sdkPrompt", target, expected: state.targetIdentity, command });
  if (result.operation !== "sdkPrompt" || !result.ok) throw new EvaluationContractError("sdk_prompt_result_required");
  return result;
}

/** Called by the existing design/story coordinator with its existing receipt and claim. */
export async function runSdkJourney(reference: ArtifactReference, claim: OutputClaim): Promise<RuntimeJourneyReceipt> {
  const receipt: RuntimeJourneyReceipt = { id: "sdk-prompt-roundtrip", proofLevel: "owned-production-runtime", pass: false,
    fixtureIds: ["main.script-list", SDK_FIXTURE_ID], assertions: [], frames: [], effects: [], cleanup: unknownOwnedCleanup(false) };
  const sdkCleanups: OwnedCleanup[] = [], nativeCleanups: OwnedCleanup[] = [];
  const check = (id: string, pass: boolean) => {
    receipt.assertions.push({ id, pass });
    if (!pass) throw new EvaluationContractError(id);
  };
  for (const caseId of ["submit", "cancel", "full", "disconnected"] as const) {
    let client: OwnedEvaluationClient | undefined, child: SdkChild | undefined;
    try {
      client = await OwnedEvaluationClient.launch(ROOT, reference, claim, receipt.fixtureIds);
      const target = await client.mount("main.script-list");
      child = await launchSdkChild(claim, caseId);
      receipt.effects.push({ caseId, sdkChild: child.provenance });
      const prompt = await required(child.prompt);
      await control(client, target, { operation: "begin", fixtureId: SDK_FIXTURE_ID, message: prompt,
        channel: caseId === "full" || caseId === "disconnected" ? caseId : "connected" });
      receipt.frames.push(await client.frame(target));
      // The SDK and coordinator issue independent real getState RPCs while the
      // prompt completion lane is live (and, for controls, full/disconnected).
      const sdkRpc = await required(child.rpc);
      const [sdkState, independent] = await Promise.all([
        client.driver.request({ type: "getState", target }),
        client.driver.request({ type: "getState", target }),
      ]);
      check(`${caseId}:independent-rpc-correlation`, sdkState.type === "stateResult" && independent.type === "stateResult" &&
        sdkState.requestId !== independent.requestId && sdkState.promptId === "1" && independent.promptId === "1");
      receipt.effects.push({ caseId, sdkRequestId: sdkRpc.requestId, appRequestId: sdkState.requestId, independentRequestId: independent.requestId });
      child.process.stdin.write(`${JSON.stringify({ ...sdkState, requestId: sdkRpc.requestId })}\n`);
      await child.process.stdin.flush();
      const rpc = await required(child.rpcEvidence);
      check(`${caseId}:sdk-rpc-does-not-complete-prompt`, rpc.rpcResolved && !rpc.promptResolved && rpc.submissionCount === 0 && rpc.received.length === 0 && rpc.state?.promptId === "1");
      receipt.effects.push({ caseId, sdkRpcObservation: rpc });
      await client.act(target, { type: "key", key: "down" });
      const key = caseId === "cancel" ? "escape" : "enter";
      let actionRefusal: string | null = null;
      try { receipt.effects.push({ caseId, action: await client.act(target, { type: "key", key }) }); }
      catch (error) { if (error instanceof DriverCommandRefused) actionRefusal = error.code; else throw error; }
      let completion = await control(client, target, { operation: "drain" });
      receipt.effects.push({ caseId, actionRefusal, completion });
      if (caseId === "full" || caseId === "disconnected") {
        check(`${caseId}:truthful-completion-failure`, !completion.completion.completed && completion.completion.receipt === null &&
          completion.completion.error === (caseId === "full" ? "channel_full" : "disconnected") && completion.messages.length === 0 && completion.forwarded === 0);
        const state = await client.driver.request({ type: "getState", target });
        check(`${caseId}:failure-preserves-rpc`, state.type === "stateResult" && state.promptId === "1");
        if (caseId === "disconnected") {
          const closed = await control(client, target, { operation: "close" });
          check(`${caseId}:retired-without-delivery`, closed.closed === true && closed.completion.retired && !closed.completion.completed && closed.forwarded === 0);
          continue;
        }
        await control(client, target, { operation: "releaseCapacity" });
        receipt.effects.push({ caseId, retry: await client.act(target, { type: "key", key: "enter" }) });
        completion = await control(client, target, { operation: "drain" });
      } else check(`${caseId}:action-not-refused`, actionRefusal === null);
      const expectedValue = caseId === "cancel" ? null : "sdk-second";
      check(`${caseId}:production-completion-payload`, completion.completion.completed && completion.completion.receipt?.sequence === 1 &&
        completion.completion.receipt.outcome.kind === (caseId === "cancel" ? "cancelled" : "submitted") &&
        completion.messages.length === 1 && completion.forwarded === 1 && completion.messages[0]?.type === "submit" &&
        completion.messages[0]?.id === "1" && completion.messages[0]?.value === expectedValue);
      // Submit leaves Arg mounted; repeat through its production owner. Cancel
      // terminates that prompt lifetime independently of native hide completion;
      // its retained completion channel must still drain only once.
      if (caseId !== "cancel") {
        try { receipt.effects.push({ caseId, duplicateAction: await client.act(target, { type: "key", key }) }); }
        catch (error) { if (!(error instanceof DriverCommandRefused)) throw error; }
      }
      const duplicate = await control(client, target, { operation: "drain" });
      check(`${caseId}:exactly-once-owner`, duplicate.messages.length === 0 && duplicate.forwarded === 1 && duplicate.completion.receipt?.sequence === 1);
      child.process.stdin.write(`${JSON.stringify(completion.messages[0])}\n`);
      await child.process.stdin.flush();
      const exit = await required(child.exitEvidence);
      const code = await required(child.process.exited);
      await required(child.readers);
      check(`${caseId}:actual-sdk-received-payload-once`, code === 0 && exit.code === 0 && exit.rpcResolved && exit.received.length === 1 &&
        exit.received[0]?.id === "1" && exit.received[0]?.value === expectedValue &&
        exit.submissionCount === (caseId === "cancel" ? 0 : 1) && exit.promptResolved === (caseId !== "cancel") &&
        exit.submittedValue === expectedValue);
      receipt.effects.push({ caseId, sdkExit: exit, completion, duplicate });
      const closed = await control(client, target, { operation: "close" });
      check(`${caseId}:completion-owner-retired`, closed.closed === true && closed.completion.retired);
    } catch (error) {
      receipt.error = [receipt.error, `${caseId}:${error instanceof Error ? error.message : String(error)}`].filter(Boolean).join("; ");
      if (error instanceof DriverLifecycleError) nativeCleanups.push(error.cleanup);
      else if (!child && error && typeof error === "object" && "cleanup" in error)
        (client ? sdkCleanups : nativeCleanups).push(error.cleanup as OwnedCleanup);
    } finally {
      // Separate owners: neither child's shutdown/drain may delay requesting
      // the other's teardown. Each closure still requires its own evidence.
      await Promise.all([
        (async () => {
          if (!child) return;
          let cleanup: OwnedCleanup;
          try { cleanup = await child.process.close(); }
          catch (error) { cleanup = unknownOwnedCleanup(true); receipt.error = `${receipt.error ?? ""};sdk_cleanup:${String(error)}`; }
          sdkCleanups.push(cleanup);
          receipt.effects.push({ caseId, sdkCleanup: cleanup });
          const drained = await boundedObservation(child.readers, 2000);
          if (drained.completed === false) receipt.error = `${receipt.error ?? ""};sdk_reader_cleanup:${String(drained.error)}`;
        })(),
        (async () => {
          if (!client) return;
          let cleanup: OwnedCleanup;
          try { cleanup = await client.close(); }
          catch (error) {
            cleanup = error instanceof DriverLifecycleError ? error.cleanup : client.cleanup;
            receipt.error = `${receipt.error ?? ""};native_cleanup:${String(error)}`;
          }
          nativeCleanups.push(cleanup);
          receipt.effects.push({ caseId, nativeCleanup: cleanup });
        })(),
      ]);
    }
  }
  receipt.cleanup = {
    ...aggregateCleanup([...sdkCleanups, ...nativeCleanups]),
    // The fixed SDK child owns no native windows. Its null is not evidence
    // about the app's windows; unknown closure from the native owner stays null.
    ownedWindowsClosed: aggregateCleanup(nativeCleanups).ownedWindowsClosed,
  };
  receipt.pass = !receipt.error && receipt.assertions.length > 0 && receipt.assertions.every(item => item.pass) && receipt.cleanup.closed;
  return receipt;
}

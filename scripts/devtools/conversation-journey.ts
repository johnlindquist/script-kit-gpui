import type { RuntimeJourneyReceipt } from "./design.ts";
import type { Json } from "./driver.ts";
import { EvaluationContractError, type FixtureControl, type OwnedEvaluationClient } from "./lib/owned-evaluation.ts";
import type { AutomationInstance } from "./lib/target-identity.ts";

export const CONVERSATION_FIXTURE_IDS = [
  "agent-chat.dense-log.retryable-failure", "agent-chat.detached.retryable-failure",
  "agent-chat.quick-ai.empty", "sdk-chat.empty", "flow.session",
] as const;

/** Extends the coordinator's existing runtime/receipt; never launches a second owner. */
export async function runConversationAcceptance(client: OwnedEvaluationClient, receipt: RuntimeJourneyReceipt): Promise<void> {
  const check = (id: string, pass: boolean) => {
    receipt.assertions.push({ id, pass });
    if (!pass) throw new EvaluationContractError(id);
  };
  const mount = async (fixture: string) => {
    const target = await client.mount(fixture);
    check(`${fixture}:hidden`, (await client.inspect(target)).windowVisible === false);
    receipt.frames.push(await client.frame(target));
    return target;
  };
  const observe = async (target: AutomationInstance, predicate: (value: Json) => boolean): Promise<Json> => {
    const deadline = performance.now() + 5000;
    for (let poll = 0; poll < 100 && performance.now() < deadline; poll++) {
      const state = await client.inspect(target);
      if (state.fixtureObservation && predicate(state.fixtureObservation)) return state.fixtureObservation;
      await client.frame(target);
    }
    throw new EvaluationContractError("conversation_production_postcondition_deadline");
  };
  const control = async (target: AutomationInstance, command: FixtureControl): Promise<Json> => {
    const before = await client.captureFrame(target, false);
    const response = await client.driver.request({ type: "design", command: {
      operation: "fixtureControl", target, expected: before.frame.target, control: command,
    } });
    if (response.result?.operation !== "fixtureControl" || response.result?.ok !== true)
      throw new EvaluationContractError(response.result?.error?.code ?? "conversation_fixture_control_failed");
    receipt.effects.push(response.result);
    return response.result.observation;
  };
  const typeText = async (target: AutomationInstance, text: string) => {
    // ChatPrompt's production TextInput consumes one scalar per key event.
    // A multi-character key_char is not a paste or an IME text insertion.
    for (const character of text) {
      receipt.effects.push(await client.act(target, {
        type: "key", key: character === " " ? "space" : character.toLowerCase(), text: character,
      }));
    }
  };
  const textOf = (message: Json): string => message.content ?? message.text ?? "";
  const drainOf = (state: Json, generation: number): Json | undefined =>
    state.drains?.find((drain: Json) => drain.streamGeneration === generation);

  for (const fixture of CONVERSATION_FIXTURE_IDS.slice(0, 3)) {
    const target = await mount(fixture);
    const quick = fixture === "agent-chat.quick-ai.empty";
    if (quick) {
      const started = await control(target, { family: "agentChat", operation: "submit", text: "Quick AI fixture request" });
      await control(target, { family: "agentChat", operation: "fail", turnGeneration: started.streamGeneration });
    }
    const failed = await observe(target, state => state.status === "error");
    check(`${fixture}:real_failed_turn`, failed.startedTurns === 1);
    check(`${fixture}:canonical_capabilities`, failed.sessionPolicy === (quick ? "QuickAi" : "Full") &&
      ["contextPortals", "localAttachments", "cwdPicker", "history", "retainedThreads", "profileSwitch"]
        .every(key => failed.capabilities?.[key] === !quick));
    const retried = await control(target, { family: "agentChat", operation: "retry" });
    check(`${fixture}:retry_starts_replacement`, retried.startedTurns === failed.startedTurns + 1 && retried.turnId > failed.turnId);
    const successText = "provider-free retry response";
    await control(target, { family: "agentChat", operation: "emitText", turnGeneration: retried.streamGeneration, text: successText });
    await control(target, { family: "agentChat", operation: "complete", turnGeneration: retried.streamGeneration });
    const success = await observe(target, state => state.status === "idle");
    check(`${fixture}:real_success_flush`, success.assistantText.includes(successText) && success.streamGeneration === retried.streamGeneration);

    // Gate and retain the ACTUAL scheduled production drain, not a rejected
    // attempt to inject an old generation through the fixture adapter.
    const old = await control(target, { family: "agentChat", operation: "submit", text: "old streaming request" });
    await control(target, { family: "agentChat", operation: "holdDrain" });
    await control(target, { family: "agentChat", operation: "emitText", turnGeneration: old.streamGeneration, text: "old turn buffered text" });
    await observe(target, state => drainOf(state, old.streamGeneration)?.queued === true);
    await control(target, { family: "agentChat", operation: "retainDrain" });
    const stopped = await control(target, { family: "agentChat", operation: "stop" });
    check(`${fixture}:local_stop_not_remote_ack`, stopped.localStreamCancelled === true &&
      stopped.remoteCancelRequested === true && stopped.remoteCancelAcknowledged === false && stopped.status !== "error");
    const replacement = await control(target, { family: "agentChat", operation: "submit", text: "replacement streaming request" });
    await control(target, { family: "agentChat", operation: "holdDrain" });
    const replacementText = "replacement buffer survives old callback";
    await control(target, { family: "agentChat", operation: "emitText", turnGeneration: replacement.streamGeneration, text: replacementText });
    await observe(target, state => drainOf(state, replacement.streamGeneration)?.queued === true);
    await control(target, { family: "agentChat", operation: "releaseDrain", turnGeneration: old.streamGeneration });
    const afterOld = await observe(target, state => drainOf(state, old.streamGeneration)?.callbacks === 1);
    const oldDrain = drainOf(afterOld, old.streamGeneration)!;
    receipt.effects.push({ operation: "productionOldTurnDrain", fixture, observation: afterOld });
    check(`${fixture}:queued_old_turn_cannot_mutate_replacement`, oldDrain.retained === true &&
      oldDrain.staleRejected === true && oldDrain.replacementStreamGeneration === replacement.streamGeneration &&
      oldDrain.replacementBufferUnchanged === true && oldDrain.replacementTaskPresent === true &&
      oldDrain.replacementTaskPreserved === true && oldDrain.replacementTranscriptUnchanged === true &&
      drainOf(afterOld, replacement.streamGeneration)?.callbacks === 0 && afterOld.status === "streaming");
    await control(target, { family: "agentChat", operation: "releaseDrain", turnGeneration: replacement.streamGeneration });
    const progressing = await observe(target, state => (drainOf(state, replacement.streamGeneration)?.callbacks ?? 0) > 0);
    check(`${fixture}:replacement_production_drain_progress`, drainOf(progressing, replacement.streamGeneration)?.staleRejected === false &&
      drainOf(progressing, replacement.streamGeneration)?.replacementBufferUnchanged === false && progressing.assistantText !== afterOld.assistantText);
    await control(target, { family: "agentChat", operation: "complete", turnGeneration: replacement.streamGeneration });
    const settled = await observe(target, state => state.status === "idle");
    check(`${fixture}:replacement_drain_remains_live`, settled.assistantText.endsWith(replacementText) &&
      settled.startedTurns === replacement.startedTurns && settled.streamGeneration === replacement.streamGeneration);
    receipt.frames.push(await client.frame(target));
    await client.unmount(target);
  }

  const sdk = await mount("sdk-chat.empty");
  const sdkInput = "SDK Chat actual local callback question";
  await typeText(sdk, sdkInput);
  const composed = (await client.inspect(sdk)).fixtureObservation;
  receipt.effects.push({ operation: "sdkChatComposedInput", observation: composed });
  check("sdk_chat:real_input_preserves_complete_text", composed?.input === sdkInput && composed.sinkRequests?.length === 0);
  receipt.effects.push(await client.act(sdk, { type: "key", key: "enter" }));
  const submitted = await observe(sdk, state => state.sinkRequests?.length === 1);
  const accepted = submitted.sinkRequests[0];
  receipt.effects.push({ operation: "sdkChatCallbackSink", observation: submitted });
  check("sdk_chat:provider_free_configuration", submitted.useBuiltinAi === false && submitted.saveHistory === false);
  check("sdk_chat:submission_reaches_actual_sink", accepted.promptId === "sdk-chat.empty" &&
    accepted.displayText === sdkInput && accepted.outboundText === sdkInput &&
    typeof accepted.payloadFingerprint === "string" && accepted.payloadFingerprint.length > 0 &&
    accepted.requestRef === submitted.acceptedRequests[0] && typeof submitted.streamingMessageId === "string");
  const failedSdk = await control(sdk, { family: "sdkChat", operation: "fail", messageId: submitted.streamingMessageId });
  check("sdk_chat:typed_failure", failedSdk.messages.some((message: Json) => message.id === submitted.streamingMessageId && !!message.failure));
  const retrySdk = await control(sdk, { family: "sdkChat", operation: "retry" });
  check("sdk_chat:retry_same_accepted_request", retrySdk.sinkRequests.length === 2 &&
    retrySdk.sinkRequests[1].requestRef === accepted.requestRef && retrySdk.sinkRequests[1].outboundText === sdkInput);
  const sdkAnswer = "SDK local streamed answer";
  await control(sdk, { family: "sdkChat", operation: "emitText", messageId: retrySdk.streamingMessageId, text: sdkAnswer });
  const completeSdk = await control(sdk, { family: "sdkChat", operation: "complete", messageId: retrySdk.streamingMessageId });
  check("sdk_chat:real_stream_complete", completeSdk.messages.some((message: Json) =>
    message.id === retrySdk.streamingMessageId && textOf(message) === sdkAnswer && message.streaming === false && !message.failure));
  const stopSdk = await control(sdk, { family: "sdkChat", operation: "submit", text: "SDK local Stop" });
  const stoppedSdk = await control(sdk, { family: "sdkChat", operation: "stop" });
  check("sdk_chat:stop_reaches_local_callback", stoppedSdk.stopRequests === 1 && stoppedSdk.messages.some((message: Json) =>
    message.id === stopSdk.streamingMessageId && message.streaming === false && !message.failure));
  receipt.frames.push(await client.frame(sdk));
  await client.unmount(sdk);

  const flow = await mount("flow.session");
  const initialFlow = (await client.inspect(flow)).fixtureObservation;
  const sessionId = initialFlow.sessionId;
  check("flow:session_identity", Number.isSafeInteger(sessionId) && sessionId > 0);
  const running = await control(flow, { family: "flow", sessionId, operation: "submit", text: "Flow background request" });
  check("flow:real_turn_started", running.state === "Working" && typeof running.activeMessageId === "string");
  const messageId = running.activeMessageId;
  await typeText(flow, "Flow next draft");
  const background = await control(flow, { family: "flow", sessionId, operation: "background" });
  check("flow:background_preserves_active_turn", background.sessionId === sessionId && background.activeMessageId === messageId &&
    background.state === "Working" && (await client.inspect(flow)).targetIdentity.appViewVariant === "FlowUxView");
  const flowText = "Flow events processed while backgrounded";
  const streamed = await control(flow, { family: "flow", sessionId, operation: "emitText", messageId, text: flowText });
  check("flow:background_event_reaches_chat_prompt", streamed.messages.some((message: Json) => message.id === messageId && textOf(message) === flowText));
  const finished = await control(flow, { family: "flow", sessionId, operation: "complete", messageId });
  check("flow:background_terminal_reducer", finished.state === "NeedsYou" && finished.activeMessageId === null &&
    finished.runtimeGeneration === running.runtimeGeneration && finished.messages.some((message: Json) =>
      message.id === messageId && textOf(message) === flowText && message.streaming === false));
  const resumed = await control(flow, { family: "flow", sessionId, operation: "resume" });
  check("flow:session_and_draft_survive_navigation", resumed.sessionId === sessionId && resumed.draftText === "Flow next draft" &&
    resumed.runtimeGeneration === running.runtimeGeneration && resumed.messages.some((message: Json) => textOf(message) === flowText));
  receipt.frames.push(await client.frame(flow));
  // Leave the last mounted surface for the coordinator's existing final capture.
}

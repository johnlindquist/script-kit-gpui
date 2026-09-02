import { expect, test } from "bun:test";
import { deflateSync, gzipSync } from "node:zlib";
import { ProtocolCore, DriverCommandRefused, PROTOCOL_VERSION, MAX_PROTOCOL_REQUEST_BYTES, OWNED_RESPONSE_CODEC, OWNED_RESPONSE_ENCODING, normalizeProtocolResponse, type Json } from "./driver.ts";

class RecordingProtocol extends ProtocolCore {
  sent: Json[] = [];
  failures: Error[] = [];
  constructor() { super(50, "protocol-test"); }
  protected writeCommand(payload: Json): void { this.sent.push(payload); }
  protected authorizeCommand(_command: Json): void {}
  protected onTransportFailure(error: Error): void { this.failures.push(error); }
  get alive(): boolean { return true; }
  async close(): Promise<void> { this.failAllPending(new Error("test closed")); }
  respond(value: Json): void { this.handleResponse(value); }
}

function encodedReply(reply: Json, decoded = Buffer.from(JSON.stringify(reply)), compressed = deflateSync(decoded, { level: 1 })): Json {
  return { type: "encodedResponse", version: 1, encoding: OWNED_RESPONSE_ENCODING,
    requestId: reply.requestId, protocolVersion: reply.protocolVersion, responseType: reply.type,
    decodedBytes: decoded.length, compressedBytes: compressed.length, payload: compressed.toString("base64") };
}

test("missing, foreign, wrong-type and wrong-version replies cannot settle a request", async () => {
  const driver = new RecordingProtocol();
  const request = driver.request({ type: "getState" });
  const requestId = driver.sent[0]!.requestId;
  for (const reply of [
    { type: "stateResult", protocolVersion: PROTOCOL_VERSION },
    { requestId: "foreign", type: "stateResult", protocolVersion: PROTOCOL_VERSION },
    { requestId, type: "elementsResult", protocolVersion: PROTOCOL_VERSION },
    { requestId, type: "stateResult", protocolVersion: 99 },
    { requestId, type: "stateResult" },
  ]) driver.respond(reply);
  expect(driver.stats.responsesMatched).toBe(0);
  driver.respond({ requestId, type: "stateResult", protocolVersion: PROTOCOL_VERSION, value: "actual" });
  expect((await request).value).toBe("actual");
  expect(driver.stats.responsesMatched).toBe(1);
});

test("bus metadata and nested response must agree before terminal settlement", async () => {
  const driver = new RecordingProtocol(); const request = driver.request({ type: "getState" });
  const requestId = driver.sent[0]!.requestId;
  const response = { requestId, type: "stateResult", protocolVersion: PROTOCOL_VERSION };
  driver.respond({ requestId, responseType: "elementsResult", protocolVersion: PROTOCOL_VERSION, response });
  driver.respond({ requestId, responseType: "stateResult", protocolVersion: 1, response });
  driver.respond({ requestId, responseType: "stateResult", protocolVersion: PROTOCOL_VERSION, response: { ...response, requestId: "foreign" } });
  expect(driver.stats.responsesMatched).toBe(0);
  driver.respond({ requestId, responseType: "stateResult", protocolVersion: PROTOCOL_VERSION, response });
  expect((await request).type).toBe("stateResult");
});

test("scheduling acceptance stays pending; terminal deferred execution settles once", async () => {
  const driver = new RecordingProtocol(); const request = driver.request({ type: "simulateGpuiEvent", event: { type: "keyDown", key: "down" } });
  const requestId = driver.sent[0]!.requestId;
  const base = { requestId, type: "simulateGpuiEventResult", protocolVersion: PROTOCOL_VERSION, success: true };
  driver.respond({ ...base, dispatchScheduled: true, dispatchCompleted: false });
  expect(driver.stats.responsesMatched).toBe(0);
  const terminal = { ...base, dispatchScheduled: false, dispatchCompleted: true, wasDeferred: true, activationProof: "not_observed" };
  driver.respond(terminal); driver.respond(terminal);
  expect((await request).wasDeferred).toBe(true); expect(driver.stats.responsesMatched).toBe(1);
});

test("a correlated typed refusal does not terminate a healthy owned transport", async () => {
  const driver = new RecordingProtocol(); const request = driver.request({ type: "simulateGpuiEvent", event: {} });
  const requestId = driver.sent[0]!.requestId;
  driver.respond({ requestId, protocolVersion: PROTOCOL_VERSION, type: "externalCommandResult", ok: false, errorCode: "stale_target" });
  await expect(request).rejects.toBeInstanceOf(DriverCommandRefused);
  expect(driver.failures).toEqual([]);
  const next = driver.request({ type: "getState" });
  driver.respond({ requestId: driver.sent[1]!.requestId, protocolVersion: PROTOCOL_VERSION, type: "stateResult" });
  await next;
});

test.each(["direct", "bus"])("%s evaluator errors reject immediately without poisoning later requests or cleanup", async transport => {
  const driver = new RecordingProtocol();
  let refusal: unknown;
  const request = driver.request({ type: "design", command: { operation: "fixtureControl" } })
    .catch(error => { refusal = error; });
  const requestId = driver.sent[0]!.requestId;
  const response = { requestId, protocolVersion: PROTOCOL_VERSION, type: "error",
    code: "stale_target_identity", message: "Owned evaluation operation refused" };
  try {
    driver.respond(transport === "direct" ? response : {
      requestId, protocolVersion: PROTOCOL_VERSION, responseType: "error", response,
    });
    await Promise.resolve();
    expect(refusal).toBeInstanceOf(DriverCommandRefused);
    expect(refusal).toMatchObject({ code: "stale_target_identity", requestId });
    await request;
    expect(driver.matchedResponses).toEqual([{ requestId, expectedType: "designResult", responseType: "error" }]);
    expect(driver.failures).toEqual([]);
    expect(driver.protocolFaults).toEqual([]);

    const next = driver.request({ type: "getState" });
    driver.respond(response);
    expect(driver.stats.responsesMatched).toBe(1);
    expect(driver.stats.unmatchedResponses).toBe(1);
    driver.respond({ requestId: driver.sent[1]!.requestId, protocolVersion: PROTOCOL_VERSION, type: "stateResult", value: "healthy" });
    expect((await next).value).toBe("healthy");
    expect(driver.stats.responsesMatched).toBe(2);
  } finally {
    await driver.close();
    await request;
  }
  expect(driver.failures).toEqual([]);
});

test("evaluator errors retain correlation, version, payload and bus identity gates", async () => {
  const driver = new RecordingProtocol();
  const request = driver.request({ type: "waitFor" });
  const requestId = driver.sent[0]!.requestId;
  const response = { requestId, protocolVersion: PROTOCOL_VERSION, type: "error",
    code: "stale_target_identity", message: "Owned evaluation operation refused" };
  for (const reply of [
    { ...response, requestId: undefined },
    { ...response, requestId: "foreign" },
    { ...response, protocolVersion: 1 },
    { ...response, protocolVersion: undefined },
    { ...response, code: undefined },
    { ...response, code: 17 },
    { ...response, message: undefined },
    { ...response, message: 17 },
    { ...response, type: "unrelatedResult" },
    { requestId, protocolVersion: PROTOCOL_VERSION, responseType: "waitForResult", response },
    { requestId, protocolVersion: 1, responseType: "error", response },
    { requestId, protocolVersion: PROTOCOL_VERSION, responseType: "error", response: { ...response, requestId: "foreign" } },
  ]) driver.respond(reply);
  expect(driver.stats.responsesMatched).toBe(0);
  expect(driver.failures).toEqual([]);
  expect(driver.protocolFaults).toContain("missing_response_request_id");
  expect(driver.protocolFaults).toContain("response_protocol_version_mismatch");
  expect(driver.protocolFaults).toContain("wrong_response_type");
  expect(driver.protocolFaults).toContain("nested_response_identity_mismatch");
  expect(driver.stats.unmatchedResponses).toBe(1);
  driver.respond(response);
  await expect(request).rejects.toBeInstanceOf(DriverCommandRefused);
  await driver.close();
  expect(driver.failures).toEqual([]);
});

test("a deferred acceptance can terminate with a correlated evaluator error", async () => {
  const driver = new RecordingProtocol();
  const request = driver.request({ type: "simulateGpuiEvent", event: {} });
  const requestId = driver.sent[0]!.requestId;
  driver.respond({ requestId, protocolVersion: PROTOCOL_VERSION, type: "simulateGpuiEventResult",
    success: true, dispatchScheduled: true, dispatchCompleted: false });
  expect(driver.stats.responsesMatched).toBe(0);
  driver.respond({ requestId, protocolVersion: PROTOCOL_VERSION, type: "error",
    code: "target_closed", message: "Owned evaluation operation refused" });
  await expect(request).rejects.toMatchObject({ code: "target_closed", requestId });
  expect(driver.stats.responsesMatched).toBe(1);
  await driver.close();
  expect(driver.failures).toEqual([]);
});

test("issued IDs cannot be reused and conflicting expectations fail before transport", async () => {
  const driver = new RecordingProtocol();
  expect(() => driver.request({ type: "getAgentChatState" }, { expect: "agentChatStateResult" })).toThrow("conflicts");
  expect(() => driver.request({ type: "unreviewed" })).toThrow("expected_response_type_required");
  const pending = driver.request({ type: "getState", requestId: "one-use" });
  driver.respond({ requestId: "one-use", protocolVersion: PROTOCOL_VERSION, type: "stateResult" }); await pending;
  expect(() => driver.request({ type: "getState", requestId: "one-use" })).toThrow("request_id_reused");
  expect(() => driver.request({ type: "getState", payload: "x".repeat(MAX_PROTOCOL_REQUEST_BYTES) })).toThrow("stdin_line_too_long");
  expect(driver.sent).toHaveLength(1);
});

test("a terminal deadline reports transport failure rather than leaving a delayed action live", async () => {
  const driver = new RecordingProtocol();
  await expect(driver.request({ type: "getState" }, { timeoutMs: 1 })).rejects.toThrow("response_timeout");
  expect(driver.failures).toHaveLength(1);
});

test("GPUI deadlines travel to the producer and timeout sends correlated cancellation", async () => {
  const driver = new RecordingProtocol();
  const before = Date.now();
  await expect(driver.request({ type: "simulateGpuiEvent", event: { type: "keyDown", key: "down" } }, { timeoutMs: 5 }))
    .rejects.toThrow("response_timeout");
  expect(driver.sent[0]!.deadlineUnixMs).toBeGreaterThanOrEqual(before);
  expect(driver.sent[0]!.deadlineUnixMs).toBeLessThanOrEqual(Date.now());
  expect(driver.sent[1]).toEqual({ type: "cancelGpuiEvent", requestId: driver.sent[0]!.requestId, protocolVersion: PROTOCOL_VERSION });
  expect(driver.failures).toHaveLength(1);
});

test("inconsistent GPUI success cannot be mistaken for completed input", async () => {
  const driver = new RecordingProtocol();
  const request = driver.request({ type: "simulateGpuiEvent", event: { type: "keyDown", key: "down" } });
  const base = { requestId: driver.sent[0]!.requestId, type: "simulateGpuiEventResult", protocolVersion: PROTOCOL_VERSION, success: true };
  driver.respond({ ...base, dispatchCompleted: false, dispatchScheduled: false });
  driver.respond({ ...base, dispatchCompleted: true, dispatchScheduled: true, wasDeferred: true });
  driver.respond({ ...base, success: false, errorCode: "dispatch_cancelled", dispatchCompleted: false, dispatchScheduled: true });
  driver.respond({ ...base, success: false, errorCode: "dispatch_cancelled" });
  expect(driver.stats.responsesMatched).toBe(0);
  driver.respond({ ...base, dispatchCompleted: true, dispatchScheduled: false, wasDeferred: true, activationProof: "not_observed" });
  await request;
  expect(driver.stats.responsesMatched).toBe(1);
});

test("legacy input version still requires canonical version two terminal replies", async () => {
  const driver = new RecordingProtocol(); const pending = driver.request({ type: "getState", protocolVersion: 1 });
  const requestId = driver.sent[0]!.requestId;
  driver.respond({ requestId, type: "stateResult", protocolVersion: 1 });
  expect(driver.stats.responsesMatched).toBe(0);
  driver.respond({ requestId, type: "stateResult", protocolVersion: PROTOCOL_VERSION });
  expect((await pending).protocolVersion).toBe(PROTOCOL_VERSION);
});

test("unsolicited lifecycle observations never settle even a matching explicit End request", async () => {
  const driver = new RecordingProtocol();
  const pending = driver.request({ type: "design", command: { operation: "end" } });
  const requestId = driver.sent[0]!.requestId;
  const lifecycle = { type: "designResult", protocolVersion: 2,
    result: { operation: "end", lifecycle: true, shutdownReason: "explicitEnd", ownedWindowsClosed: true } };
  for (const value of [lifecycle, { ...lifecycle, requestId },
    { ...lifecycle, requestId, result: { ...lifecycle.result, lifecycle: false } },
    { responseType: "designResult", protocolVersion: 2, response: lifecycle }]) driver.respond(value);
  expect(driver.stats.responsesMatched).toBe(0);
  const explicit = { type: "designResult", protocolVersion: 2, requestId,
    result: { operation: "end", ok: true, ownedWindowsClosed: true } };
  driver.respond(explicit);
  expect(await pending).toEqual(explicit);
  expect(driver.stats.responsesMatched).toBe(1);
  expect(driver.protocolFaults).toEqual(Array(4).fill("unexpected_native_lifecycle"));
});

test.each(["direct", "bus"])("%s negotiated encoding restores every response field before routing", async transport => {
  const driver = new RecordingProtocol();
  driver.enableResponseEncoding(OWNED_RESPONSE_ENCODING);
  const pending = driver.request({ type: "design", command: { operation: "captureFrame" } });
  const requestId = driver.sent[0]!.requestId;
  expect(driver.sent[0]!.responseEncoding).toBe(OWNED_RESPONSE_ENCODING);
  const response = { type: "designResult", protocolVersion: PROTOCOL_VERSION, requestId, result: {
    operation: "captureFrame", ok: true, frame: { processInstanceId: "owned", target: { frameGeneration: 19 } },
    snapshot: { pngBase64: "AAECAwQ=", correlationId: requestId },
    state: { selectedSemanticId: "résumé", rows: Array(80).fill({ label: "same metadata", selected: false }) },
    frameEvidence: { searchMetadataRef: 0, paintBindings: [{ kind: "mainSearch", id: "main-search", metadata: { query: "résumé" } }] },
  } };
  const before = JSON.stringify(response); const encoded = encodedReply(response);
  driver.respond(transport === "direct" ? encoded : { requestId, protocolVersion: PROTOCOL_VERSION, responseType: response.type, response: encoded });
  expect(await pending).toEqual(response);
  expect(JSON.stringify(response)).toBe(before);
  expect(encoded.compressedBytes).toBeLessThan(encoded.decodedBytes);
  expect(driver.matchedResponses).toEqual([{ requestId, expectedType: "designResult", responseType: "designResult" }]);
  expect(driver.protocolFaults).toEqual([]);
});

test("per-request encoding does not alter legacy responses or depend on another response", async () => {
  const driver = new RecordingProtocol();
  const first = driver.request({ type: "getState", responseEncoding: OWNED_RESPONSE_ENCODING });
  const second = driver.request({ type: "getState", responseEncoding: OWNED_RESPONSE_ENCODING });
  const responses = driver.sent.map((command, index) => ({ type: "stateResult", protocolVersion: PROTOCOL_VERSION,
    requestId: command.requestId, value: `independent-${index}` }));
  driver.respond(encodedReply(responses[1]!)); driver.respond(encodedReply(responses[0]!));
  expect(await first).toEqual(responses[0]); expect(await second).toEqual(responses[1]);
  const legacy = driver.request({ type: "getState" });
  expect(driver.sent[2]!.responseEncoding).toBeUndefined();
  const identity = { type: "stateResult", protocolVersion: PROTOCOL_VERSION, requestId: driver.sent[2]!.requestId };
  driver.respond(identity); expect(await legacy).toEqual(identity);
});

test("encoded scheduling acceptance stays pending and a typed refusal preserves the transport", async () => {
  const driver = new RecordingProtocol(); driver.enableResponseEncoding(OWNED_RESPONSE_ENCODING);
  const pending = driver.request({ type: "simulateGpuiEvent", event: {} });
  const requestId = driver.sent[0]!.requestId;
  driver.respond(encodedReply({ type: "simulateGpuiEventResult", protocolVersion: PROTOCOL_VERSION, requestId,
    success: true, dispatchScheduled: true, dispatchCompleted: false }));
  expect(driver.stats.responsesMatched).toBe(0);
  driver.respond(encodedReply({ type: "error", protocolVersion: PROTOCOL_VERSION, requestId,
    code: "stale_target_identity", message: "Owned evaluation operation refused" }));
  await expect(pending).rejects.toBeInstanceOf(DriverCommandRefused);
  expect(driver.failures).toEqual([]);
  const next = driver.request({ type: "getState" });
  driver.respond(encodedReply({ type: "stateResult", protocolVersion: PROTOCOL_VERSION, requestId: driver.sent[1]!.requestId }));
  await next; expect(driver.stats.responsesMatched).toBe(2);
});

test("encoding opt-in and mandatory encoded delivery fail closed without transport fallback", async () => {
  for (const value of [undefined, null, 1, {}, "gzip-json-base64-v1"]) {
    const driver = new RecordingProtocol();
    expect(() => driver.request({ type: "getState", responseEncoding: value })).toThrow("response_encoding_invalid");
    expect(driver.sent).toEqual([]);
  }
  for (const optedIn of [false, true]) {
    const driver = new RecordingProtocol();
    if (optedIn) driver.enableResponseEncoding(OWNED_RESPONSE_ENCODING);
    const pending = driver.request({ type: "getState" });
    const response = { type: "stateResult", protocolVersion: PROTOCOL_VERSION, requestId: driver.sent[0]!.requestId };
    driver.respond(optedIn ? response : encodedReply(response));
    const code = optedIn ? "response_encoding_missing" : "response_encoding_unrequested";
    await expect(pending).rejects.toMatchObject({ code });
    expect(driver.failures).toHaveLength(1); expect(driver.stats.responsesMatched).toBe(0);
  }
});

test("malformed encoding headers lengths base64 and decoded identity never reach a caller", async () => {
  const mutations: Array<[string, (value: Json) => void, string]> = [
    ["extra field", value => { value.extra = true; }, "response_encoding_invalid_header"],
    ["missing field", value => { delete value.decodedBytes; }, "response_encoding_invalid_header"],
    ["version", value => { value.version = 2; }, "response_encoding_invalid_header"],
    ["codec", value => { value.encoding = "zlib-json-base64-v2"; }, "response_encoding_invalid_header"],
    ["protocol", value => { value.protocolVersion = 1; }, "response_encoding_invalid_header"],
    ["decoded bound", value => { value.decodedBytes = OWNED_RESPONSE_CODEC.maxDecodedBytes + 1; }, "response_encoding_invalid_header"],
    ["compressed bound", value => { value.compressedBytes = OWNED_RESPONSE_CODEC.maxCompressedBytes + 1; }, "response_encoding_invalid_header"],
    ["fractional length", value => { value.decodedBytes += 0.5; }, "response_encoding_invalid_header"],
    ["zero length", value => { value.compressedBytes = 0; }, "response_encoding_invalid_header"],
    ["null payload", value => { value.payload = null; }, "response_encoding_invalid_header"],
    ["whitespace", value => { value.payload += "\n"; }, "response_encoding_invalid_base64"],
    ["invalid alphabet", value => { value.payload = "!" + value.payload.slice(1); }, "response_encoding_invalid_base64"],
    ["padding bits one byte", value => { value.payload = "YR=="; value.compressedBytes = 1; }, "response_encoding_invalid_base64"],
    ["padding bits two bytes", value => { value.payload = "YWJ="; value.compressedBytes = 2; }, "response_encoding_invalid_base64"],
    ["interior padding", value => { value.payload = "Y=Q="; value.compressedBytes = 2; }, "response_encoding_invalid_base64"],
    ["compressed length", value => { value.compressedBytes += 3; }, "response_encoding_invalid_base64"],
    ["decoded underclaim", value => { value.decodedBytes -= 1; }, "response_encoding_invalid_stream"],
    ["decoded overclaim", value => { value.decodedBytes += 1; }, "response_encoding_length_mismatch"],
    ["response type identity", value => { value.responseType = "elementsResult"; }, "response_encoding_identity_mismatch"],
    ["nested codec", value => { value.responseType = "encodedResponse"; }, "response_encoding_invalid_header"],
  ];
  for (const [name, mutate, code] of mutations) {
    const driver = new RecordingProtocol();
    const pending = driver.request({ type: "getState", responseEncoding: OWNED_RESPONSE_ENCODING });
    const encoded = encodedReply({ type: "stateResult", protocolVersion: PROTOCOL_VERSION, requestId: driver.sent[0]!.requestId, value: name });
    mutate(encoded); driver.respond(encoded);
    await expect(pending).rejects.toMatchObject({ code });
    expect(driver.stats.responsesMatched).toBe(0); expect(driver.failures).toHaveLength(1);
  }
});

test("zlib checksum truncation dictionaries and trailing streams are not accepted", async () => {
  for (const corrupt of [
    (compressed: Buffer, _decoded: Buffer) => { const changed = Buffer.from(compressed); changed[changed.length - 1] = changed[changed.length - 1]! ^ 255; return changed; },
    (compressed: Buffer) => compressed.subarray(0, compressed.length - 1),
    (compressed: Buffer) => Buffer.concat([compressed, Buffer.from("trailing")]),
    (compressed: Buffer) => Buffer.concat([compressed, compressed]),
    (_compressed: Buffer, decoded: Buffer) => gzipSync(decoded),
    (_compressed: Buffer, decoded: Buffer) => deflateSync(decoded, { dictionary: Buffer.from("requestId protocolVersion stateResult") }),
  ]) {
    const driver = new RecordingProtocol();
    const pending = driver.request({ type: "getState", responseEncoding: OWNED_RESPONSE_ENCODING });
    const response = { type: "stateResult", protocolVersion: PROTOCOL_VERSION, requestId: driver.sent[0]!.requestId };
    const decoded = Buffer.from(JSON.stringify(response));
    driver.respond(encodedReply(response, decoded, corrupt(deflateSync(decoded), decoded)));
    await expect(pending).rejects.toMatchObject({ code: "response_encoding_invalid_stream" });
    expect(driver.stats.responsesMatched).toBe(0);
  }
});

test("invalid UTF-8 JSON and outer-inner request identities are rejected after bounded inflate", async () => {
  for (const body of [Buffer.from([255, 254]), Buffer.from("{invalid"), Buffer.from("null"), Buffer.from("[]"),
    Buffer.from(JSON.stringify({ type: "stateResult", protocolVersion: PROTOCOL_VERSION, requestId: "foreign" }))]) {
    const driver = new RecordingProtocol();
    const pending = driver.request({ type: "getState", responseEncoding: OWNED_RESPONSE_ENCODING });
    driver.respond(encodedReply({ type: "stateResult", protocolVersion: PROTOCOL_VERSION, requestId: driver.sent[0]!.requestId }, body));
    await expect(pending).rejects.toMatchObject({ code: body[0] === 255 || body.toString() === "{invalid"
      ? "response_encoding_invalid_json" : "response_encoding_identity_mismatch" });
    expect(driver.stats.responsesMatched).toBe(0);
  }
});

test("decoded size is bounded before parsing while a complete response at the limit roundtrips", async () => {
  const driver = new RecordingProtocol(); driver.enableResponseEncoding(OWNED_RESPONSE_ENCODING);
  const pending = driver.request({ type: "getState" });
  const response = { type: "stateResult", protocolVersion: PROTOCOL_VERSION, requestId: driver.sent[0]!.requestId, value: "" };
  response.value = "x".repeat(OWNED_RESPONSE_CODEC.maxDecodedBytes - Buffer.byteLength(JSON.stringify(response)));
  const encoded = encodedReply(response);
  expect(encoded.decodedBytes).toBe(OWNED_RESPONSE_CODEC.maxDecodedBytes);
  driver.respond(encoded); expect((await pending).value).toBe(response.value);
  const bomb = driver.request({ type: "getState" });
  const oversized = encodedReply({ ...response, requestId: driver.sent[1]!.requestId }, Buffer.alloc(OWNED_RESPONSE_CODEC.maxDecodedBytes + 1, 120));
  oversized.decodedBytes = OWNED_RESPONSE_CODEC.maxDecodedBytes;
  driver.respond(oversized);
  await expect(bomb).rejects.toMatchObject({ code: "response_encoding_invalid_stream" });
});

test("encoded bus routing still requires every outer identity to match", async () => {
  const driver = new RecordingProtocol();
  const pending = driver.request({ type: "getState", responseEncoding: OWNED_RESPONSE_ENCODING });
  const requestId = driver.sent[0]!.requestId;
  const response = encodedReply({ type: "stateResult", protocolVersion: PROTOCOL_VERSION, requestId });
  const envelope = { requestId, protocolVersion: PROTOCOL_VERSION, responseType: "stateResult", response };
  for (const wrong of [{ ...envelope, requestId: "foreign" }, { ...envelope, protocolVersion: 1 },
    { ...envelope, responseType: "encodedResponse" }]) driver.respond(wrong);
  expect(driver.stats.responsesMatched).toBe(0);
  driver.respond(envelope); expect((await pending).type).toBe("stateResult");
  expect(driver.protocolFaults).toEqual(Array(3).fill("nested_response_identity_mismatch"));
});

test("unsolicited lifecycle stays separate from the encoded explicit End response", async () => {
  const driver = new RecordingProtocol(); driver.enableResponseEncoding(OWNED_RESPONSE_ENCODING);
  const pending = driver.request({ type: "design", command: { operation: "end" } });
  driver.respond({ type: "designResult", protocolVersion: PROTOCOL_VERSION,
    result: { operation: "end", lifecycle: true, shutdownReason: "explicitEnd", ownedWindowsClosed: true } });
  expect(driver.stats.responsesMatched).toBe(0);
  expect(driver.protocolFaults).toEqual(["unexpected_native_lifecycle"]);
  driver.respond(encodedReply({ type: "designResult", protocolVersion: PROTOCOL_VERSION, requestId: driver.sent[0]!.requestId,
    result: { operation: "end", ok: true, ownedWindowsClosed: true } }));
  expect((await pending).result.ok).toBe(true);
});

test("physical response normalization preserves legacy records and complete decoded response identity", () => {
  const original = { type: "stateResult", protocolVersion: PROTOCOL_VERSION, requestId: "physical-record",
    targetIdentity: { windowId: "owned", frameGeneration: 19 },
    searchObservation: { rawInput: "résumé", rows: [{ selected: true, pixel: { r: 10, g: 20, b: 30 } }] } };
  for (const legacy of [original, { kind: "protocolResponse", response: original }, null, [], "log message", 17])
    expect(normalizeProtocolResponse(legacy as Json)).toBe(legacy);
  const encoded = Object.freeze(encodedReply(original)); const physical = JSON.stringify(encoded);
  expect(normalizeProtocolResponse(encoded)).toEqual(original);
  expect(JSON.stringify(encoded)).toBe(physical);
});

test("physical response normalization never downgrades malformed encoded evidence to a legacy record", () => {
  const original = { type: "stateResult", protocolVersion: PROTOCOL_VERSION, requestId: "physical-record" };
  for (const invalid of [
    { ...encodedReply(original), version: 2 },
    { ...encodedReply(original), payload: "invalid base64" },
    { ...encodedReply(original), decodedBytes: OWNED_RESPONSE_CODEC.maxDecodedBytes + 1 },
    { ...encodedReply(original), responseType: "elementsResult" },
  ]) {
    const before = JSON.stringify(invalid);
    expect(() => normalizeProtocolResponse(invalid)).toThrow("response_encoding_");
    expect(JSON.stringify(invalid)).toBe(before);
  }
});

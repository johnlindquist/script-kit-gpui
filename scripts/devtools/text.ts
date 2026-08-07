#!/usr/bin/env bun
/** Text fingerprint/length measurement. Shared transport/args/receipts live in lib/client.ts. */

import {
  type JsonObject,
  asArray,
  classifyEnvelopes,
  finishReceipt,
  parseTargetArgs,
  requestId,
  responseOf,
  rpc,
  startClock,
} from "./lib/client.ts";
import { emitValidatedReceipt } from "./lib/receipt-schema.ts";
import { diagnostic } from "./lib/privacy.ts";
import { maybeStartAndShow, resolveTargetReceipt } from "./lib/target-identity.ts";

function usage() {
  return "Usage:\n  bun scripts/devtools/text.ts measure [target args] [--limit <n>]";
}

function textOf(node: JsonObject) {
  const text = node.text ?? node.value ?? "";
  return typeof text === "string" ? text : String(text);
}

function fingerprint(value: string) {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

function textRows(nodes: JsonObject[]) {
  return nodes
    .map((node) => {
      const text = textOf(node);
      return {
        semanticId: node.semanticId ?? null,
        role: node.role ?? node.type ?? null,
        contentKind: "UserContent",
        redacted: true,
        textLength: text.length,
        lineCount: text.length ? text.split(/\r\n|\r|\n/).length : 0,
        selected: node.selected ?? null,
        focused: node.focused ?? null,
        fingerprint: fingerprint(text),
      };
    })
    .filter((row) => row.textLength > 0);
}

function classify(targetReceipt: JsonObject, stateEnvelope: JsonObject, elementsEnvelope: JsonObject, rows: ReturnType<typeof textRows>) {
  if (targetReceipt.classification !== "ok") {
    return targetReceipt.classification ?? "blocked-by-target-ambiguity";
  }
  const transport = classifyEnvelopes([stateEnvelope, elementsEnvelope]);
  if (transport !== "ok") {
    return transport;
  }
  if (rows.length === 0) {
    return "blocked-by-missing-primitive";
  }
  return "ok";
}

async function main() {
  const argv = Bun.argv.slice(2);
  if (argv[0] !== "measure") {
    if (argv.includes("--help") || argv.includes("-h")) {
      console.log(usage());
      process.exit(0);
    }
    console.error(usage());
    process.exit(2);
  }
  const { args, extras, warnings: argWarnings } = parseTargetArgs(argv.slice(1), { extras: { "--limit": "number" } });
  if (args.help) {
    console.log(usage());
    process.exit(0);
  }
  const limit = extras["--limit"] ?? 120;

  const clock = startClock();
  await maybeStartAndShow(args);
  const targetReceipt = await resolveTargetReceipt(args, { tool: "text" });
  const selector = (targetReceipt.requestedTarget as JsonObject | undefined)?.selector ?? args.target ?? { type: "focused" };
  const stateEnvelope = await rpc(args.session, {
    type: "getState",
    requestId: requestId("text", "state"),
    target: selector,
    summaryOnly: true,
  }, "stateResult", args.timeoutMs);
  const elementsEnvelope = await rpc(args.session, {
    type: "getElements",
    requestId: requestId("text", "elements"),
    target: selector,
    limit,
  }, "elementsResult", args.timeoutMs);
  const state = responseOf(stateEnvelope);
  const elements = responseOf(elementsEnvelope);
  const nodes = asArray(elements.elements);
  const rows = textRows(nodes);
  const inputValue = typeof state.inputValue === "string" ? state.inputValue : "";
  const selectedValue = typeof state.selectedValue === "string" ? state.selectedValue : "";
  const footerButtons = asArray((state.activeFooter as JsonObject | undefined)?.buttons);
  const footerTexts = footerButtons.map((button) => ({
    action: button.action ?? null,
    key: button.key ?? null,
    label: button.label ?? null,
    labelLength: typeof button.label === "string" ? button.label.length : null,
  }));
  const classification = classify(targetReceipt, stateEnvelope, elementsEnvelope, rows);

  emitValidatedReceipt("devtools.text.measure", finishReceipt(
    { tool: "script-kit-devtools.text", command: "text.measure", session: args.session, clock },
    {
      classification,
      requestedTarget: targetReceipt.requestedTarget ?? { selector },
      target: targetReceipt.resolvedTarget ?? null,
      textSummary: {
        contentKind: "UserContent",
        rawContentReturned: false,
        inputLength: inputValue.length,
        inputFingerprint: fingerprint(inputValue),
        selectedLength: selectedValue.length,
        selectedFingerprint: fingerprint(selectedValue),
        textNodeCount: rows.length,
        longestTextLength: rows.reduce((max, row) => Math.max(max, row.textLength), 0),
      },
      rows,
      footerTexts,
      missingPrimitives: [
        rows.length === 0 ? "textNodes" : "",
        rows.some((row) => row.semanticId == null) ? "semanticId" : "",
        rows.length > 0 ? "textBounds" : "",
        stateEnvelope.status === "error" ? "stateResult" : "",
        elementsEnvelope.status === "error" ? "elementsResult" : "",
        targetReceipt.classification !== "ok" ? "strictTargetIdentity" : "",
      ].filter(Boolean),
      warnings: [
        ...argWarnings,
        "Text bounds are not exposed by getElements yet; pair this with devtools.layout.measure for geometry.",
      ],
      errors: diagnostic([
        ...((targetReceipt.errors as JsonObject[]) ?? []),
        ...[stateEnvelope, elementsEnvelope].filter((value) => value.status === "error"),
      ]),
    },
  ));
}

await main();

#!/usr/bin/env bun
/**
 * Target discovery and strict target-identity receipts. The identity pipeline
 * lives in lib/target-identity.ts; this file is the CLI wrapper. Inspector
 * CLIs no longer shell out to this tool — they call resolveTargetReceipt()
 * in-process.
 */

import {
  classifyEnvelopes,
  finishReceipt,
  hasSessionLifecycleError,
  lifecycleCodes,
  parseTargetArgs,
  primaryLifecycleDetails,
  primaryParsedError,
  primarySessionLifecycle,
  requestId,
  responseOf,
  rpc,
  startClock,
} from "./lib/client.ts";
import { emitValidatedReceipt } from "./lib/receipt-schema.ts";
import { diagnostic } from "./lib/privacy.ts";
import { maybeStartAndShow, pickWindows, resolveTargetReceipt } from "./lib/target-identity.ts";

function usage() {
  return [
    "Usage:",
    "  bun scripts/devtools/targets.ts list [--session <name>] [--start] [--show]",
    "  bun scripts/devtools/targets.ts inspect --target-id <id>|--target-kind <kind>|--main|--focused [--surface <SurfaceKind>] [--strict]",
  ].join("\n");
}

async function main() {
  const argv = Bun.argv.slice(2);
  const command = argv[0] === "inspect" ? "inspect" : "list";
  const { args, extras, warnings } = parseTargetArgs(
    argv[0] === "inspect" || argv[0] === "list" ? argv.slice(1) : argv,
    { extras: { "--hi-dpi": "boolean" } },
  );
  if (args.help) {
    console.log(usage());
    process.exit(0);
  }

  const clock = startClock();
  await maybeStartAndShow(args);

  if (command === "list") {
    const windowsEnvelope = await rpc(
      args.session,
      { type: "listAutomationWindows", requestId: requestId("targets", "list") },
      "automationWindowListResult",
      args.timeoutMs,
    );
    const windows = responseOf(windowsEnvelope);
    const errors = [windowsEnvelope].filter((value) => value.status === "error");
    emitValidatedReceipt(
      "devtools.targets.list",
      finishReceipt(
        { tool: "script-kit-devtools.targets", command: "targets.list", session: args.session, clock },
        {
          classification: hasSessionLifecycleError(errors)
            ? "blocked-by-session-lifecycle"
            : classifyEnvelopes(errors),
          lifecycleCodes: lifecycleCodes(errors),
          lifecycleDetails: diagnostic(primaryLifecycleDetails(errors)),
          sessionLifecycle: primarySessionLifecycle(errors),
          parsedError: diagnostic(primaryParsedError(errors)),
          targetCount: pickWindows(windows).length,
          targets: pickWindows(windows),
          warnings,
          errors: diagnostic(errors),
        },
      ),
    );
    return;
  }

  const receipt = await resolveTargetReceipt(args, {
    tool: "targets",
    hiDpi: Boolean(extras["--hi-dpi"]),
  });
  emitValidatedReceipt(
    "devtools.targets.inspect",
    finishReceipt(
      { tool: "script-kit-devtools.targets", command: "targets.inspect", session: args.session, clock },
      {
        ...receipt,
        lifecycleDetails: diagnostic(receipt.lifecycleDetails),
        parsedError: diagnostic(receipt.parsedError),
        rawInspect: diagnostic(receipt.rawInspect),
        errors: diagnostic(receipt.errors),
        warnings,
      },
    ),
  );
}

await main();

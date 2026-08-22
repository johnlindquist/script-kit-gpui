# scripts/devtools — receipt-first app instrumentation

CLIs and a library for inspecting and driving a running script-kit-gpui app
over its stdin/stdout JSON protocol. Every CLI prints exactly one JSON
receipt to stdout with a fail-closed `classification`.

## Entry points

```bash
bun scripts/devtools/devtools.ts list                 # dispatcher: all tools
bun scripts/devtools/devtools.ts elements snapshot --main --start
bun scripts/devtools/targets.ts inspect --focused    # or call files directly
```

Two transports:

- **Session CLIs** (`targets`, `elements`, `focus`, `keyboard`, `text`,
  `scroll`, `layout`, `surface`, `act`, `events`, ...) talk to a
  `scripts/agentic/session.sh` session (FIFO + response file, ~0.5–2s per
  command). Good for one-shot receipts and cross-process workflows.
- **Driver** (`driver.ts`) is event-driven over pipes (~10–50ms per step).
  `Driver.launch()` owns a fresh app process (sandboxed HOME available);
  `Driver.attach({session})` joins a running session.sh session and never
  kills the app. Use the driver for multi-step probes and fast loops.

```bash
bun scripts/devtools/driver.ts smoke                  # launch + rpc timing proof
bun scripts/devtools/driver.ts attach-smoke default   # join a running session
```

## Sessions

Session CLIs default to the shared session `default`. For parallel loops pass
`--session <unique>` or set `SCRIPT_KIT_DEVTOOLS_SESSION`; using `--start`
on the implicit shared session emits a receipt warning. Sessions live under
`/tmp/sk-agentic-sessions/<name>` (override root with `SCRIPT_KIT_SESSION_DIR`).

```bash
bash scripts/agentic/session.sh start my-probe        # start/reuse a session
bash scripts/agentic/session.sh health my-probe
bash scripts/agentic/session.sh stop my-probe
```

## Receipts

All migrated CLIs build through `lib/client.ts` `finishReceipt`, then must
emit through `lib/receipt-schema.ts` `emitValidatedReceipt`. The latter is the
only public print boundary and produces `ReceiptEnvelopeV2`: stable primitive,
run/task, repository/producer, binary, fixture/transaction, recursive privacy,
evidence-layer, assertion, interference, cleanup, disposition, and producer
validation fields around each tool-specific body. Only `EVALUABLE_PASS` exits
zero; blocked receipts exit 3 and invalid receipts exit 4. The classification
vocabulary lives in `schema.ts` — notable values:

- `ok` / `reproduced` / `fixed` / `not-reproduced` — proof outcomes
- `blocked-by-session-lifecycle` — session/forwarder/app process is gone;
  restart the session, don't retry the CLI
- `blocked-by-session-queue`, `blocked-by-response-timeout`,
  `blocked-by-parse-error` — precise transport failures
- `blocked-by-missing-primitive` — the app didn't expose what the tool needs

## Target selection (shared flags)

`--session <name> --target-id <id> | --target-kind <kind> [--target-index n]
| --target-title <text> | --target-json <json> | --focused | --main`
plus `--strict`, `--surface <SurfaceKind>`, `--timeout <ms>`, `--start`,
`--show`. Parsed by `lib/client.ts` `parseTargetArgs`; target resolution
happens in-process via `lib/target-identity.ts` (no subprocess hop).

## Library layout

- `lib/client.ts` — transport (`run`, `rpc`), arg parsing, receipt envelope,
  error classification, binary fingerprint. Start here for a new CLI.
- `lib/target-identity.ts` — window listing/inspection and strict target
  identity (`resolveTargetReceipt`, `maybeStartAndShow`).
- `lib/transport-errors.ts` — session.sh error-code → classification map.
- `driver.ts` — `ProtocolCore` (typed protocol surface) + `Driver` (owned
  process) + `AttachedDriver` (running session). Both support `await using`.

## Tests

```bash
cd scripts/devtools && bun test ./__tests__/
```

## Gotchas

- Codex-imp/seatbelt sandboxes cannot launch the GUI app. Launch the session
  outside the sandbox, then attach (`Driver.attach` / session CLIs) from
  inside. A wall of rpc timeouts right after a sandboxed launch is
  `blocked-by-sandbox`, not an app bug.
- Never run bare `cargo` here while `./dev.sh` may be running — build via
  `./scripts/agentic/agent-cargo.sh` (see CLAUDE.md).
- The driver picks the freshest of `target/debug` and the agent-cargo pool
  binary and prints which it chose on stderr; pin with
  `SCRIPT_KIT_GPUI_BINARY` or the `binary` option.

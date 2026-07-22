# Flow UX Protocol (frozen 2026-07-09)

Contract between the mdflow CLI (~/dev/mdflow) and the Script Kit GPUI flow
launcher surfaces. Both sides build against this document; change it only by
bumping `protocolVersion` and keeping a fallback path.

`protocolVersion: 1` everywhere below.

## 1. `md roster --json`

One-shot. Prints a single JSON object to stdout, exit 0.

```jsonc
{
  "protocolVersion": 1,
  "cwd": "/abs/path",              // process cwd at invocation
  "projectRoot": "/abs/path|null", // resolved project root (config > flows/ > git root > cwd)
  "flows": [
    {
      "id": "project:review",         // stable: "<source>:<slug>", slug = filename stem
      "path": "/abs/path/flows/review.md",
      "source": "project" | "global" | "registry",
      "name": "review",               // filename stem
      "description": "…|null",        // frontmatter description
      "engine": "pi",                 // resolved via the normal ladder, config-aware
      "engineSource": "filename" | "frontmatter" | "config" | "default",
      "inputs": [                     // from _inputs, [] when none
        { "name": "target", "type": "text|select|number|confirm|password",
          "message": "…|null", "options": ["…"] /* select only */,
          "default": null }
      ],
      "isWorkflow": false,            // has _steps
      "interactive": false,           // _interactive / .i. marker
      "mtimeMs": 1752000000000
    }
  ]
}
```

- Ordering: project flows first (alphabetical), then global, then registry.
- A markdown file with no frontmatter and no engine marker (a "document") is
  excluded from the roster.
- Errors (no project, unreadable dir) still exit 0 with `flows: []` and a
  `warnings: ["…"]` array when applicable.

## 2. `md explain <flow> --json`

Free (no engine call). Single JSON object on stdout.

```jsonc
{
  "protocolVersion": 1,
  "flowId": "project:review",
  "path": "/abs/path/flows/review.md",
  "engine": "pi",
  "command": "pi",                 // executable
  "args": ["--print", "…"],        // full argv (prompt may be elided, see promptIncluded)
  "cwd": "/abs/path",              // effective run cwd (_cwd applied)
  "prompt": "…",                   // fully resolved prompt body
  "promptTokensEstimate": 1234,    // rough estimate, chars/4 acceptable
  "inputs": [ …same shape as roster… ],
  "warnings": ["…"],
  "configFingerprint": "sha256:…"  // hash over resolved config + flow content + mdflow version
}
```

Cache key on the app side: `(path, mtimeMs, cwd, mdflowVersion, configFingerprint)`.

## 3. `md <flow> --events` (run event stream)

NDJSON on **stdout only** — stdout is protocol-pure; every line is one JSON
object terminated by `\n`. Human/rendered output is suppressed. Engine
provider output is carried **inside** `output.delta` events (JSON-escaped),
never interleaved raw. Diagnostics may go to stderr as free text.

Common envelope on every event:

```jsonc
{ "protocolVersion": 1, "seq": 0, "runId": "r-<uuid>", "ts": 1752000000000, "event": "…", …payload }
```

`seq` starts at 0, increments by 1 per event, no gaps. Event order contract:
`protocol` first, `run.started` second *when a run actually starts*, terminal
event (`run.completed` | `run.error` | `run.cancelled`) last, exactly one
terminal. Pre-start failures (interactive rejection, missing file, missing
required inputs, document files) legitimately emit `protocol` → `run.error`
with **no** `run.started` — no pid exists. Consumers must fail closed: a
stream that ends without a terminal event is an error, and a missing
`run.completed.exitCode` is never success.

| event | payload |
|---|---|
| `protocol` | `{ "mdflowVersion": "4.1.0" }` |
| `run.started` | `{ "flowId", "path", "engine", "command", "args", "cwd", "pid" }` |
| `output.delta` | `{ "channel": "stdout"\|"stderr", "text": "…" }` |
| `step.started` | `{ "stepId", "needs": ["…"] }` (workflows only) |
| `step.completed` | `{ "stepId", "exitCode", "cached": false }` |
| `run.completed` | `{ "exitCode": 0, "durationMs": 1234 }` |
| `run.error` | `{ "exitCode": 42\|null, "message": "…", "durationMs": 1234 }` (nonzero exit or spawn failure) |
| `run.cancelled` | `{ "signal": "SIGTERM", "durationMs": 1234 }` |

- `--events` implies non-interactive. A TTY-only interactive flow emits
  `run.error` with `message: "interactive flow requires a terminal"` — the app
  then offers "Open in Terminal" instead of pretending it can host it.
- Inputs: the app collects `_inputs` values natively (Lens/forms) and passes
  them as `--_<name> <value>` overrides; `--events` runs never prompt.
- Cancellation: app sends SIGTERM to the **process group**; mdflow forwards to
  the engine child, emits `run.cancelled`, exits. App escalates to SIGKILL on
  the group after a bounded wait (2s) and verifies descendants are gone.

### Capability handshake

Before first use of any contract above, the app runs `md roster --json`.
Classification is deliberate, not blanket:

- Parsed JSON with `protocolVersion` = 1 → `ready`.
- Parsed JSON with a different `protocolVersion` → `legacy`.
- Nonzero exit whose stderr shows `roster` being resolved as a flow name
  ("not found" naming `roster`) → `legacy` (pre-protocol mdflow).
- Any other nonzero exit or parse failure → `error`, with a stderr excerpt
  surfaced in `roster.warnings` — a broken config must never hide behind a
  calm "legacy mdflow" banner.

Legacy mode is honest: the app shows the upgrade path (`npm i -g mdflow`)
and disables protocol-driven launch. There is no silent `--json` blob
fallback (`run.started.pid` for workflows is mdflow's own pid; single runs
report the engine child's pid — informational only, never a signal target).

## 4. App-side domain model (src/flows/)

```
RunPhase       = Starting | Running | Succeeded | Failed | Cancelled
EngagementMode = Inline | Background | ManagerFocused
```

Phase and engagement are independent axes. `Esc` on an engaged run changes
engagement to `Background`; it never cancels. Cancel is an explicit action on
the selected run only.

Output tails are bounded: registry keeps at most 64 KiB / 500 lines per run
(newest wins); full output is not retained in app state.

## 5. Shared interaction grammar (all variations)

| Key | Action |
|---|---|
| `Enter` | launch with the variation's primary lifecycle |
| `Shift+Enter` | launch in background |
| `Cmd+Enter` | launch and focus Flow Manager |
| `Esc` (engaged run) | background the run (never cancel) |
| explicit Cancel action | cancel selected run only |
| "New chat from run" | new warm Pi Agent Chat seeded with run context; labeled as new chat, never as continuation |
| `⌥←` / `⌥→` | cycle between Flow UX variants while inside one |

## 6. Automation state (getState exposure)

Under a `flowUx` key in the devtools state snapshot:

```jsonc
{
  "activeVariant": "flash"|"dispatch"|"lens"|"missionControl"|null,
  "selectedFlowId": "project:review"|null,
  "roster": { "status": "ready"|"loading"|"legacy"|"error", "count": 12, "cwd": "/…" },
  "preview": { "flowId": "…", "fingerprint": "sha256:…", "valid": true } | null,
  "runs": [
    { "runId": "r-…", "flowId": "…", "phase": "Running", "engagement": "Background",
      "selected": false, "exitCode": null, "outputTail": "last line…",
      "launchAckMs": 42, "spawnMs": 180, "firstOutputMs": 610 }
  ],
  "manager": { "visible": true, "focusedRunId": "r-…"|null }
}
```

Redaction: password-type input values never appear in state, logs, semantic
elements, or screenshots; only lengths may be reported.

## 7. Semantic IDs

Built-ins (hidden, QueryOnly visibility — excluded only for the empty query):

- `flow-ux-flash` → "Flow UX — Flash"
- `flow-ux-dispatch` → "Flow UX — Dispatch"
- `flow-ux-lens` → "Flow UX — Lens"
- `flow-ux-mission-control` → "Flow UX — Mission Control"

The detached Flow Manager window was removed (2026-07-22, dead-surface
cleanup): run supervision lives in the main-window Flow Desk. Closing the
launcher never cancels runs.

# Script Kit v1 → v2 Migration Engine

Agent-driven porting of `~/.kenv`-era (v1) scripts to the v2 SDK, gated by a
mechanical validator ladder. The agent does ALL nontrivial porting; the ladder —
not the model — is what earns trust. Copy, never move: v1 sources are read-only.

## Usage

```bash
# Free, read-only triage against the compat map
bun scripts/migrate/cli.ts scan ~/.kenv/scripts

# Full pipeline: port → validate → repair loop → verified files + report
bun scripts/migrate/cli.ts port ~/.kenv/scripts
# defaults to --out ~/.scriptkit/plugins/v1-imports/scripts (its own plugin,
# visible in the launcher, reversible by deleting the folder)

bun scripts/migrate/cli.ts port <dir|file> \
  [--out <dir>] [--dry-run] [--no-exec] [--force-agent] \
  [--max-repairs 3] [--no-honesty] [--concurrency 4] [--json]
```

Agent backend: `claude -p --output-format json` by default. Override with
`SK_MIGRATE_AGENT_CMD` — any command that reads a prompt on stdin and writes
the response to stdout (a Claude print-mode JSON envelope is unwrapped
automatically, plain text works too).

## Per-script flow

```
classify → port (agent; `ready` scripts are copied verbatim unless --force-agent)
         → validator ladder → repair loop (raw validator output fed back, ≤3 tries)
         → honesty refute pass (only when a needs-rewrite port claims zero
           behavior changes)
         → verified file + migration-report.json, or needs-review with receipts
```

## The validator ladder (cheapest first, short-circuits to repair)

1. **typecheck** — real `tsc` compiling the ported file TOGETHER with
   `scripts/kit-sdk.ts`, whose `declare global` blocks provide the ambient
   globals. `--moduleDetection force` mirrors bun's every-file-is-a-module
   semantics. Catches hallucinated APIs.
2. **api-scan** — the classifier re-run on the agent's OUTPUT. Fails on any
   `removed`/`stub`/`renamed` API or a surviving `@johnlindquist/kit` import;
   `caveat` APIs warn.
3. **metadata** — launcher-visible fields (`// Name:`, `// Shortcut:`, typed
   `metadata = {}`, ...) must survive with identical values ("typed wins,
   comments fill gaps", mirroring `src/scripts/metadata.rs`).
4. **smoke** — runs the script for real (`bun --preload scripts/kit-sdk.ts`,
   sandboxed `SK_PATH`). Pass = a valid first JSONL protocol message on stdout,
   or a clean exit for prompt-less scripts.
5. **walkthrough** — full run with `SDK_TEST_AUTOSUBMIT=1`: the SDK
   auto-resolves every prompt, so a well-behaved script terminates. Exit 0 =
   pass, crash = fail, timeout = warn (inconclusive, some scripts run long).
6. **honesty** — a second agent prompted to REFUTE a suspicious
   zero-behavior-change claim. Only runs when the original used
   removed/stub APIs, i.e. where silent amputation is plausible.

⚠️ Validators 4–5 EXECUTE the script — real filesystem/network side effects
run (SK_PATH is sandboxed, nothing else is). Use `--no-exec` to skip them.

## Design invariants

- **`compat-map.json` is the single source of truth.** The classifier, the
  api-scan validator, and the port prompt all read it, so the agent and the
  validators can never disagree about what is allowed. When the v2 SDK grows
  an API, update the map — everything downstream follows.
- **Validators speak, the agent fixes.** Repair prompts contain the raw tool
  output (tsc diagnostics, scanner lines, script stderr), never a paraphrase.
- **Receipts everywhere.** Every attempt's verdicts land in
  `migration-report.json`; `needs-review` results carry the full failure
  detail so an Agent Chat handoff can join the conversation mid-flight
  instead of starting over.
- Prompts live in `prompts/*.md` as versioned templates — the fix for a bad
  port is usually a compat-map entry or a prompt tweak, not engine code.

## Tests

```bash
bun test ./scripts/migrate/__tests__/   # from the repo root
```

Includes live integration coverage: real tsc runs, real `bun --preload`
smoke/walkthrough runs against the actual SDK (including the case a crash
*after* the first prompt slips past smoke and must be caught by walkthrough).

## Next phases (not built yet)

- Launcher-native migration board built-in (list rows + receipts + "Accept all
  verified"), replacing the silent `migrate_from_kenv()` startup move.
- "Port with AI" Agent Chat handoff seeded from a `needs-review` report entry.

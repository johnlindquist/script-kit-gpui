# AI phase trace

Every AI surface — Quick AI, Agent Chat, Text, Mini, Flows — emits one
comparable NDJSON lifecycle trace, so latency questions are answered with
measurements instead of impressions.

```bash
bash scripts/agentic/ai-phase-trace-check.sh          # re-check everything, offline
bash scripts/agentic/ai-phase-trace-check.sh --probe  # also drive live turns first
```

Expected tail: five `PASS:` lines, the per-surface table, then
`AI_PHASE_TRACE_CHECK=PASS`.

Enable tracing in any run with `SCRIPT_KIT_AI_TRACE_PATH=/path/trace.ndjson`.
Unset, the trace is a single null check per event — no clock, no filesystem, no
hashing.

## Pieces

| Path | Role |
|---|---|
| `src/ai/phase_trace.rs` | The shared writer, vocabulary, and redaction posture |
| `src/ai/agent_chat/pi/runtime.rs` | Pi transport (Agent Chat, Text, Mini) |
| `src/flows/codex_client.rs` | Flows transport (`codex app-server`) |
| `src/ai/agent_chat/codex_exec.rs` | Quick AI, mirrored onto the shared trace |
| `ai-phase-trace-report.ts` | Trace → per-surface numbers and verdict |
| `ai-phase-trace-probe.ts` | Drives real turns through the real app |
| `fixtures/ai-phase-trace-receipt.ndjson` | The committed measured run |
| `fixtures/ai-phase-trace-mini-per-turn-runid.ndjson` | Mini attributable after the run-id fix |
| `fixtures/ai-phase-trace-prewarm-on.ndjson` | The disproved prewarm variant |
| `fixtures/ai-phase-trace-flow.ndjson` | Flows measured over `codex app-server` |

## The verdict rule

Two **independent** axes, because the repairs differ — a long turn needs less
work, a silent one needs earlier feedback:

- `actually-slow` — median turn ≥ 5000 ms
- `feels-slow` — median time-to-first-**visible**-output ≥ 1500 ms
- no verdict below **n = 5** valid samples

Reasoning/thought tokens are not visible output. Counting them would flatter any
surface that streams its thinking. Failed and cancelled turns are excluded from
every median: a turn that failed in 492 ms is not a 492 ms surface, and without
that exclusion a broken surface ranks fastest in the table.

## Measured, 2026-07-25 (four surfaces, real turns)

Receipts: `fixtures/ai-phase-trace-receipt.ndjson` (Quick AI, Agent Chat,
Text), `fixtures/ai-phase-trace-mini-per-turn-runid.ndjson` (Mini, post-fix),
`fixtures/ai-phase-trace-flow.ndjson` (Flows). They are separate files because
no single app session can drive all five surfaces.

| Surface | Transport | n | valid | first event | first visible | total | Verdict |
|---|---|---|---|---|---|---|---|
| quick-ai | codex-exec | 8 | 8 | 143 ms | 3121 ms | 3121 ms | **feels-slow** |
| agent-chat | pi-rpc | 6 | 6 | 4356 ms | 4356 ms | 4598 ms | **feels-slow** |
| text | pi-rpc | 6 | 6 | 3680 ms | 3680 ms | 3905 ms | **feels-slow** |
| mini | pi-rpc | 12 | 5 | — | — | — | **ambiguous-trace (defect since fixed)** |
| flow | codex-app-server | 11 | 10 | 1951 ms | 1951 ms | 2035 ms | **feels-slow** |

**All five surfaces are now measured, and not one of them is actually slow.
Every single one feels slow.** Every median turn is under the 5 s bar; every one
shows the user nothing for 94–100% of that turn. The shared symptom is dead air,
not duration — so the shared repair is earlier feedback, not more speed. Flows
is the fastest surface at 2.0 s and still fails the same way.

But the dead air has **two different causes**, and they need different fixes:

- **Quick AI — silent by presentation.** The first provider byte lands at
  **143 ms**; the answer is then buffered and revealed only at the end, so
  first-visible equals total. The transport is already fast. Making the model
  faster cannot fix this: a 30% speedup still leaves ~2.2 s of blank screen,
  still over the bar. The fix is to reveal progressively.
- **Pi surfaces (Agent Chat, Text, Mini) — silent by cold start.** Here
  first-event *equals* first-visible at 3.7–4.4 s: nothing arrives at all, so
  there is nothing to reveal earlier. A raw `pi --mode rpc` turn outside the
  app spends **3697 ms before its first line** and only ~1.0 s on the answer,
  which accounts for nearly the whole number. `agent_chat_hot_prewarm_enabled`
  is **off by default** (`reason = "disabled_by_default"`), so each surface
  pays a cold sidecar spawn.

### Mini was unmeasurable, and now is not — fixed 2026-07-25

The receipt above still carries the defect, on purpose, as the regression
witness. `runId` was not unique per turn on the Pi transport: every Mini turn
was recorded under the constant `"pi-isolated"`, and Mini is the one surface
that fans out into **concurrent** turns — one rewrite submit fires several
variation turns at once. Measured from that receipt, Mini reached **3 concurrent
open turns** under a single id and left **2 unterminated**, against 12
`turn_start`s and only 10 `terminal`s.

Both halves are now repaired:

1. **The transport cannot produce the collision.** `PhaseTrace::begin_at`
   suffixes the caller's label with a process-global turn ordinal, so the id is
   minted at construction rather than trusted to each call site. The label
   survives as a prefix (`pi-isolated#2`), so the transport is still readable.
   Enforcing it at the constructor is the point: the call sites already got
   this wrong once.
2. **The analyzer refuses instead of averaging.** It now checks for overlapping
   turn lifetimes *before* splitting at `turn_start`, and any surface with
   overlap or an unterminated turn gets the `ambiguous-trace` verdict — medians
   withheld from the table, and excluded from `MEASURED_SURFACES`. The refusal
   is scoped to the offending surface so one bad surface cannot erase good
   numbers beside it.

Proof, live, same probe: Mini went from **12 turns / 5 valid /
`ambiguous-trace`** to **12 turns / 12 valid / `feels-slow`**, with 12 unique
run ids for 12 turn starts. Receipt:
`fixtures/ai-phase-trace-mini-per-turn-runid.ndjson`.

The lesson worth keeping: the old analyzer could not detect this and so
published a confident median for a turn nobody ran. An instrument that cannot
tell concurrent turns apart must say so, not average the wreckage.

Post-fix Mini reads **4867 ms first visible / 4960 ms total — feels-slow**, in
line with Text on the same transport. Do not compare those absolutes against
the table above; they are a different run on a differently loaded machine. The
result being claimed here is the attribution fix, not a latency change.

## Prewarming does not fix the Pi surfaces — it makes them worse

The obvious inference from "cold spawn dominates" is "prewarm the sidecar".
Measured, that is wrong. Receipt: `fixtures/ai-phase-trace-prewarm-on.ndjson`.

| Text surface | first visible | total | Verdict |
|---|---|---|---|
| prewarm off (default) | 3680 ms | 3905 ms | feels-slow |
| prewarm on | 5559 ms | 5636 ms | **actually-and-feels-slow** |

Turning hot prewarm on made Text **~1.7 s slower** and pushed it over the
actually-slow bar. That is consistent with why it was disabled: idle Pi workers
"can consume multiple CPU cores and starve GPUI frame delivery"
(`agent_chat_launch.rs`). The prewarmed sidecar wins back spawn time and then
loses more than it won to CPU contention.

Honest limit: this is n=5 vs n=6 in one sitting, not the paired 15-trial design
in `quick-ai-latency-bench.ts`. The direction is large (+48%) and mechanistically
explained, but treat the exact delta as indicative, not settled.

## Flows: measured 2026-07-25, and the fastest surface

Flows is now driven live. It is the **quickest** of the five — 1951 ms to first
visible output, 2035 ms total — but it lands on the same verdict as every other
surface, because 96% of that turn is still dead air.

Driving it needs one thing that is easy to get wrong. The app resolves its flow
roster by running `md roster --json` in **the Spine cwd**, which defaults to
`$HOME/.scriptkit` — *not* the process working directory. Setting the app's cwd
does nothing; the probe seeds a fixture flow into the sandbox's
`~/.scriptkit/flows/` instead.

That fixture is deliberately a throwaway `ping.md` that replies with one word
and is told to touch nothing. **Never point this probe at the repo's own
`flows/**`.** Those are real delegation briefs; "measuring" one would set a
Codex agent loose on this checkout rather than time a turn.

## Resolved: the model-switch signal was benign

Earlier runs showed `modelSwitchPending: true` and it was left explicitly
unverified because every traced turn had failed. Healthy Pi turns are now
measured, and the field behaves as the code specifies — the guard at
`pi/runtime.rs:337` memoises `applied_model`, and `:344` clears it only on
failure. No per-message blocking `set_model` cost exists.

## Environment hazard: the pinned sidecar cannot serve the default model

`DEFAULT_PI_MODEL` is `gpt-5.6-sol` (`profiles.rs:16`), but the repo-pinned
sidecar (`pi 0.1.16`, `prepare-pi-sidecar.sh` ref `3d1a3950…`) advertises 23
models and **none of the `gpt-5.6-*` family**. A default Agent Chat launch dies
with `Model openai-codex/gpt-5.6-sol not found` — under the real `HOME`, not
just a sandbox. Text and Mini are unaffected because they pin
`gpt-5.3-codex-spark`, which the sidecar does have.

The probe therefore takes `--model` and writes it to the sandbox's
`config.ts` as `ai.selectedModelId`. That keeps the environment fact out of the
numbers **without editing a product default**, which is deliberate: whether the
app or the pinned sidecar is the stale side is the owner's call, not the
measurement's.

## Adding a surface

Call `PhaseTrace::begin` at the honest user-perceived start — before any setup
round trip, or the trace hides exactly the cost it exists to measure — then
route every event through that transport's single emission choke point. Do not
sprinkle trace calls at individual send sites; the next edit will forget one,
and a missing event produces an empty trace, which is indistinguishable from a
fast surface.

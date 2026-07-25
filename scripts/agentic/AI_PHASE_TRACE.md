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

Receipt: `fixtures/ai-phase-trace-receipt.ndjson` (30 turns, 0 corrupt lines).

| Surface | Transport | n | valid | first event | first visible | total | Verdict |
|---|---|---|---|---|---|---|---|
| quick-ai | codex-exec | 8 | 8 | 143 ms | 3121 ms | 3121 ms | **feels-slow** |
| agent-chat | pi-rpc | 6 | 6 | 4356 ms | 4356 ms | 4598 ms | **feels-slow** |
| text | pi-rpc | 6 | 6 | 3680 ms | 3680 ms | 3905 ms | **feels-slow** |
| mini | pi-rpc | 12 | 5 | 3980 ms | 3980 ms | 4067 ms | **feels-slow** |
| flow | codex-app-server | 0 | 0 | — | — | — | unmeasured |

**No surface is actually slow. All four measured surfaces feel slow.** Every
median turn is under the 5 s bar; every one of them shows the user nothing for
94–100% of that turn. The shared symptom is dead air, not duration — so the
shared repair is earlier feedback, not more speed.

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

## Why Flows is still unmeasured

Flows is wired, unit-proven, and emitting, but has no live entry path in the
probe: driving it needs a real flow plus a `codex app-server` session that this
probe does not yet open. It reports `insufficient-data` rather than inventing a
number. `MEASURED_SURFACES=4/5` is the honest scoreboard.

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

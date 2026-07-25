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

## Measured, 2026-07-25

| Surface | n valid | first byte | first visible | total | Verdict |
|---|---|---|---|---|---|
| quick-ai | 9 | 84 ms | 2363 ms | 2363 ms | **feels-slow** |
| agent-chat | 0 | — | — | — | unmeasured |
| text / mini / flow | 0 | — | — | — | unmeasured |

**Quick AI is not slow. It is silent.** The provider's first byte lands at
84 ms and the turn finishes in 2363 ms — under the slow bar — but the user sees
nothing for the whole 2363 ms, a ~100% dead-air ratio, because the answer is
buffered and revealed at the end. So the measured repair is *not* "make it
faster": a 30% speedup still leaves ~1.7 s of silence, still over the
feels-slow bar. Show something during the turn.

This also corrects an earlier characterisation of Quick AI as a ~6 s surface;
that figure bundled web-search turns. Simple queries run at 2.4 s.

## Why four surfaces are unmeasured

Agent Chat produced real traced turns — the instrument works end to end — but
each terminated `RuntimeClosed`: the Pi sidecar exits under the probe's sandbox
`HOME` even with `~/.pi` and `~/.codex` seeded (`pi_rpc_stdout_closed`, no
stderr hint). That is an environment gap, not a product defect and not a
regression. Text, Mini, and Flows are wired and unit-proven but have no live
entry path in the probe yet.

`MEASURED_SURFACES=n/5` in the report output is the honest scoreboard.

## Open, explicitly unverified

`modelSwitchPending: true` appeared on both traced Agent Chat turns. That is
**not** evidence that every message pays a blocking `set_model`: the guard at
`pi/runtime.rs:337` memoises `applied_model` and skips the round trip when the
model is unchanged, and `:344` clears the memo only on failure — and both
traced turns failed. A second *healthy* turn should read `false`. Unverified
until Pi runs under the probe.

## Adding a surface

Call `PhaseTrace::begin` at the honest user-perceived start — before any setup
round trip, or the trace hides exactly the cost it exists to measure — then
route every event through that transport's single emission choke point. Do not
sprinkle trace calls at individual send sites; the next edit will forget one,
and a missing event produces an empty trace, which is indistinguishable from a
fast surface.

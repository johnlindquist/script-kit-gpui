# Overnight handoff — AI chat parity + Flow chords (2026-07-25)

Written at 04:15 by the lane that did the chat-parity work, for you at 8am.
Read the first two sections; the rest is reference.

---

## Read this first

You went to sleep after answering an alignment page. It picked
**`parity-backlog`** at **depth 100** (docs, probes, edge cases) with
**autonomy 100** ("run it, and move into nearby work"). That is what got built.

**Everything asked for is done and committed locally. Nothing is pushed.**

Two lanes worked in this repo overnight. They never touched the same file:

| Lane | What it did | Where |
| --- | --- | --- |
| **This one** | One shared conversation renderer, so Flow answers are selectable; four parity ports | `src/components/conversation_*`, `src/prompts/chat/**`, `src/render_builtins/flow_ux.rs` |
| **w6W:p6** | A shared AI phase trace + a Quick AI latency benchmark | `src/ai/phase_trace.rs`, `src/ai/agent_chat/pi/**`, `src/flows/codex_client.rs`, `scripts/agentic/quick-ai-latency-bench*` |

They are one story: this lane made the two chat surfaces **render and behave**
the same; w6W:p6 made them **report timing** the same. Neither is blocked on
the other.

**One thing needs your decision before more work happens — see
[Decisions for you](#decisions-for-you).**

---

## The one command that proves this lane's work

```bash
./scripts/agentic/agent-cargo.sh test --bin script-kit-gpui flow_
```

Expect **90 passed, 0 failed**. It is deterministic and safe to run while
other things are building.

There is also a runtime probe, which is what actually proves the headline
claim (a Flow answer is *selectable text*, which no Rust test can see):

```bash
SCRIPT_KIT_AGENT_ARTIFACT_NAME=ai-chat-parity \
  ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui \
  && bun scripts/agentic/ai-chat-parity-probe.ts
```

Expect **8/8**. ⚠️ **Run it on a quiet machine.** See
[The probe is load-sensitive](#the-probe-is-load-sensitive) — this is the one
thing in this handoff that will waste your morning if you skip it.

---

## What shipped, in order

All nine commits are this lane's. Files listed are the whole footprint.

| # | Commit | What it does |
| --- | --- | --- |
| 1 | `634c8489b` | Promotes the shared conversation style + text seam out of Agent Chat into `src/components/`. `agent_chat/ui/style_contract.rs` becomes a façade with zero production values. |
| 2 | `beae5034d` | Ports **per-turn copy** and **jump-to-latest** from Flow into Agent Chat. |
| 3 | `dde4ce1d8` | **The headline.** Flow answers now render through the shared selectable `TextView` instead of Flow's own markdown engine, which had no concept of selection. You can finally drag-select part of a Flow answer. |
| 4 | `ef86c1375` | **New Conversation on ⌘L** in Flow, through the same rethread transaction failure-recovery uses. Refused with a neutral toast while a turn is in flight. |
| 5 | `8a7fde3e9` | Adds the runtime probe + its pure evidence module. |
| 6 | `2e92ced4b` | Spec + GLOSSARY for the shared renderer; corrects three findings the original audit got wrong. |
| 7 | `ad83ec083` | Makes the probe *actually run* — it was written but had never executed successfully. Four separate reasons why, all now documented in the code. |
| 8 | `399efa10b` | **Binds ⇧⌘C in Flow.** It was printed in the ⌘K menu and bound to nothing. |
| 9 | `18790d80e` | Docs for #8 and the class of bug it belongs to. |

### The single most useful thing to understand

Three of these bugs were the same bug wearing different clothes:

> **A value is degraded, the degraded value still looks plausible, and the
> code that would have warned never runs.**

- Flow's answers rendered *fine* — just unselectable. Nothing was red.
- ⇧⌘C had a shortcut badge in the menu and no binding. Clicking worked, so
  **every test was green** and the chord was silently dead.
- Copying mid-stream grabs the in-flight turn, whose body is `""` — and
  **writing `""` to the clipboard succeeds**. You get a copy that "worked"
  and paste nothing.

That last one is why `flows::session::resolve_last_copyable_response` returns
`Option` instead of `String`: it forces every caller to tell "no answer yet"
apart from "an answer".

The generalized guard is `every_advertised_session_shortcut_has_a_declared_owner`
in `src/render_builtins/actions.rs`. Every shortcut the Flow ⌘K menu
advertises must now name an owner, and the resolver-owned ones get actually
pressed. **Add a badge without a binding and a test fails** — the step that
got skipped.

Writing that guard turned up a second thing: `⌘⇧D` Background is *not* in the
resolver documented as "the single exhaustive key owner". It is bound, by a
window-level interceptor in `src/app_impl/startup.rs`. Not a user-visible bug,
but the doc comment overclaimed, so the guard names the exception explicitly
rather than pretending one owner covers everything.

---

## Where w6W:p6's work sits

Five commits, interleaved by time with this lane's, **zero file overlap**:

| Commit | What |
| --- | --- |
| `efd52cf5d` | Paired phase-aware Quick AI latency benchmark (`scripts/agentic/quick-ai-latency-bench.ts` + test) |
| `b721e50bb` | The shared phase trace itself (`src/ai/phase_trace.rs`) |
| `2bd87134e` | Pi turn phases for Agent Chat, Text, and Mini (`src/ai/agent_chat/pi/**`, `launch.rs`) |
| `ee4ce3874` | Flow turn phases through one event choke point (`src/flows/codex_client.rs`) |
| `edeea97d7` | Quick AI mirrored onto the shared trace, plus the trace probe/report/check tooling (`src/ai/agent_chat/codex_exec.rs`, `scripts/agentic/ai-phase-trace-*`) |

Both lanes touch `src/ai/agent_chat/` but **different subtrees** — this lane
owns `ui/`, w6W:p6 owns `pi/`, `codex_exec.rs`, and the transports. Nothing was
coordinated by editing across that line.

As of 04:20 **that lane's work is fully committed** and the tree is clean. The
only untracked path left is `.hitl-align/`, which holds the alignment page and
your submission — deliberately untracked, not leftovers.

### Earlier tonight, before either lane

For context, these landed before the parity batch and you already saw them
verified: `9028268cf` (one Stop chord + Copy Last Response across surfaces),
`48308e994` (Up/Down prompt recall in Flow), `32aac4798` and `b2fc53747`
(pasted/programmatic newline corruption), `29dc1658a` (footer audit repair).

---

## Decisions for you

**1. The composer split is still unresolved, and it blocks task #20.**

Flow composes in the *shared single-line main input* that ScriptList also
uses, where `Enter` must keep meaning "select". Multi-line + Shift+Enter
therefore requires giving Flow **its own composer**. That is a real
architectural fork, not a preference, and it is the reason #20 never started.
Nothing overnight touched it.

**2. Escape still means different things on the two surfaces.**

Escape *stops the model* in Agent Chat and *backgrounds the session* in Flow —
so a habit learned on one surface walks away from a running, spending turn on
the other. `docs/specs/backgrounded-ai-sessions.md` designs the fix (an Escape
modal + click-away backgrounding) but has **five blocking gaps G1–G5** and was
deliberately not started. `⌘.` now stops on both surfaces and both footers say
so, which is the safe half of the fix.

**3. Agent Chat's own ⇧⌘C has the empty-copy bug this lane just fixed in Flow.**

The `Cmd+Shift+C` block in `src/ai/agent_chat/ui/view.rs` (~L15196) takes the
newest assistant message unconditionally, so it *can* copy an empty body
mid-stream. Flow's path can't
anymore. Fixing it is ~10 lines against `resolve_last_copyable_response`, but
it sits in territory adjacent to the other lane's active work, so it was left
alone rather than risking a collision at 4am.

---

## Unfinished, and known-broken

Nothing in this lane's scope is half-done. These are things found *along the
way* and deliberately not started:

| Item | Where | Note |
| --- | --- | --- |
| Flow's jump pill uses a shadow flag | `src/prompts/chat/render_core.rs`, `user_has_scrolled_up` (4 sites, pill decision ~L412) | Reads that flag instead of `ListState::is_following_tail()`, the single follow-tail authority Agent Chat uses. Two sources of truth for one question. |
| **The whole Agent Chat probe family is dead** | `scripts/agentic/*agent-chat*` | Every probe calling `openAgentChatKitchenSinkFixture` fails at its first request — the fixture was deleted in `401936c41`. This lane dropped its Agent Chat half rather than fake the coverage. **This is the biggest gap in runtime coverage right now.** |
| Stale transcript source audit | `transcript_list_state_starts_with_existing_messages` | Asserts `ListState::new(total, ListAlignment::Bottom`, but `71055d11e` changed the call to `Self::list_alignment_for(anchor)`. |
| `of38` is wall-clock fragile | `flows::session::tests::of38` | Fails at load average ~8+, passes alone. Not caused by this work — `git diff` adds zero lines to the function it tests. |
| 42 pre-existing `tests/source_audits` failures | — | **Present at `29dc1658a` too.** Proven, not assumed: verified by running the built test binaries against a pristine worktree of the base commit. `tests/sdk_automation_runtime` also fails to compile at base. |

---

## The probe is load-sensitive

Found while writing this handoff, so it is fresh and worth your attention.

`ai-chat-parity-probe.ts` passed 8/8 cleanly several times tonight, including
a negative control. Re-running it just now at **load average 13.9**, with the
other lane's benchmark pegging a core, it failed **3 of 5 runs** — in two
distinct clusters:

- **the ⌘L leg** — the session closes to the main list instead of staying
  open, and the draft is lost;
- **the turn itself** — the answer never arrives inside the poll window, so
  the selectable and copy assertions cascade.

Both are timing, not product defects: the same binary passed 8/8 on an
immediate rerun, and the deterministic Rust tests never wavered.

**Confirmed before wind-down.** Once the neighbouring lane finished and load
dropped to ~5, the same binary passed **2 of 2 runs, 8/8 each**. So the rule is
measured, not guessed:

| Load average | Result |
| --- | --- |
| ~13.9 (both lanes building) | 3 of 5 runs failed |
| ~5 (quiet) | 2 of 2 runs passed, 8/8 |

**Run the probe when the machine is quiet.** If it fails, check `uptime` before
believing it.

One real weakness surfaced from this: `flowSession.cmd-l-clears-the-transcript`
**passes vacuously when the session closes entirely** — zero answer regions is
also what a dead session looks like. Only `flowSession.still-open-after-reset`
tells the two apart. If that leg is ever tightened, keep both.

---

## Where the working notes live

The full decision ledger — every branch taken, every wrong turn, receipts per
claim — is at:

```
.notes/oracle/ai-chat-parity-backlog/ledger.md
```

⚠️ `.notes/` is gitignored **twice** (repo `.gitignore:77` and your global
`~/.gitignore_global:20`), so it exists on this machine only and will never
reach a remote. That is why this handoff is a tracked file instead.

Design context: `docs/specs/one-conversation-experience.md` §5 "Landed"
(owner table + the three invariants), and `GLOSSARY.md` rows "Shared
conversation renderer" and "Flow sessions (Threadline)".

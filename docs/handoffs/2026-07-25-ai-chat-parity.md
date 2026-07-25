# Handoff — AI chat parity, Flow chords, test debt (2026-07-25)

Written at 04:15 by the lane that did the chat-parity work, updated at 12:05
after a morning of test-debt work. Read the first two sections; the rest is
reference.

---

## Morning update (added 12:05) — read this before the overnight section

After you answered the second page at 09:24, this lane did three things.

**1. Acted on your web-choices submission.** Only one of your four selections
was implemented: `agentchat-copy`. Agent Chat's `⇧⌘C` copied an empty string
mid-answer, because the in-flight turn is in the list with an empty body and
writing `""` to the clipboard SUCCEEDS. It now uses the same
`resolve_last_copyable_response` Flow uses (`823ba3da1`).

The other three were NOT started, deliberately:

- **`escape-modal`** — the option you picked says the five gaps G1–G5 need your
  answers. Selecting the path is not answering them; building it unattended
  meant guessing five times.
- **`own-composer`** — you edited that answer to add *"If it makes sense. Then
  all ai chats should share Flow's new composer."* That is conditional and
  widens the scope to every AI surface. Not an unattended call.
- **`rebuild-fixture`** — ⚠️ **my page under-informed this choice.** Commit
  `401936c41` removed those fixtures deliberately, as *"WP6: remove
  production-wired kitchen-sink fixtures"*; `src/ai/agent_chat/ui/kitchen_sink_fixture.rs`
  (383 lines) was **production** code. Restoring it re-introduces exactly what
  that commit set out to delete. The real fork is "re-add production-wired" vs
  "build a test-only fixture", and you approved a sentence that never showed
  you that trade-off.

**2. Paid down test debt** — the two items this doc listed as nobody's.

| Target | Was | Now |
| --- | --- | --- |
| `tests/sdk_automation_runtime` | **could not compile** | **39 / 39** |
| `tests/agent_chat_transcript_render_contract` | 8 / 1 | **9 / 0** |
| `tests/source_audits` | 711 / **42 failed** | 718 / **35 failed** |

Seven audits fixed, **no invariant weakened**. Every one was a false red where
production had moved *toward* the contract and the audit failed it for that:
two demanded a hardcoded `px(28.0)` and a generic hints helper while the code
had adopted the shared `GRID_GLYPH_SCALE` token and the truthful
`…_with_primary_label("Paste")`; one failed ScriptList for using a fifth
shared-chrome variant missing from a hardcoded list of four; one required
`match` arms that had become a better lookup table. Commits `2534164dd`,
`62d518396`, `273eb7808`, `d0bb3a433`, `bc90ae65f`.

**3. Hit a shared-worktree collision — one commit is misattributed.**
`5ce5b3530` carries this lane's message but contains the **sibling lane's**
phase-trace work. Two agents share one working tree and one git index; w6W:p6
staged and committed in the window between this lane's `git add` and its
`git commit`. Their work is intact and correct — only the message is wrong. It
was **not** rewritten: it already had a descendant and that lane was still
committing. Read `5ce5b3530` as "phase-trace tooling, by the sibling lane", and
`bc90ae65f` as the audit fix its message describes.

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

For the morning's test-debt work, these two are deterministic and both green:

```bash
./scripts/agentic/agent-cargo.sh test --test sdk_automation_runtime          # 39/39
./scripts/agentic/agent-cargo.sh test --test agent_chat_transcript_render_contract  # 9/9
```

---

## What I could NOT prove

Stated plainly, because every other number in this doc has a receipt and these
do not.

1. **The Agent Chat `⇧⌘C` fix has no runtime proof.** It compiles, and 797
   agent_chat + 90 flow tests pass, but nothing exercised the actual chord
   against a real window — because every Agent Chat probe still dies on the
   fixture deleted in `401936c41`. The *rule* it now calls has four unit tests
   including the mid-stream case; the *wiring* is unverified at the layer of
   the claim.
2. **The full test suite was never green in one run.** 35 source audits remain
   red (pre-existing), so "all tests pass" is not a claim I can make. I
   verified per-target instead, and every number here names its target.
3. **`of38` remains wall-clock fragile.** It failed again today at load 23.4
   and passed alone. I have never seen it fail on a quiet machine, but I also
   cannot prove it is purely environmental.
4. **The jump-pill convergence (F1) was not attempted.** It changes what
   renders, and the only tool that could prove it is the load-sensitive probe,
   on a machine at load 23 with you away. Deliberately left.
5. **Whether the 35 remaining audits are each individually correct.** I
   verified the causes of the ones I fixed. For the rest I confirmed the code
   genuinely lacks what they assert, but I did not judge whether each assertion
   is still the *right* contract — some may be stale requirements rather than
   real regressions.

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

**Status as of 12:05.** You answered these on the 09:24 page
(`.hitl-choice/sk-gpui-lane-status/submissions/3f6b8c2b-…6b54.json`). Picking a
path is recorded; three of the four still need input before anything can be
built. The exact asks:

| # | You chose | What is still needed from you |
| --- | --- | --- |
| 1 Composer | Give Flow its own composer | Resolve your own *"if it makes sense"*, and confirm the widened scope: should **all** AI chats share Flow's new composer, or Flow first? |
| 2 Escape | Build the Escape modal from the spec | Answers to gaps **G1–G5** in `docs/specs/backgrounded-ai-sessions.md`. Nothing can start without them. |
| 3 Copy | Fix reusing the Flow rule | **Done** — `823ba3da1`. No decision outstanding. |
| 4 Coverage | Rebuild the Agent Chat fixture | Re-add it **production-wired** (undoing `401936c41`'s intent) or build a **test-only** fixture? My page never showed you this trade-off. |

The original write-ups follow.

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
| ~~Stale transcript source audit~~ | — | **FIXED** `273eb7808`. Its message described row count while its code checked alignment; a negative control proved the old form would have passed a real zero-row regression. Now 9/9. |
| `of38` is wall-clock fragile | `flows::session::tests::of38` | Fails at load average ~8+, passes alone. Not caused by this work — `git diff` adds zero lines to the function it tests. Failed again today at load 23.4, passed alone. |
| ~~`sdk_automation_runtime` does not compile~~ | — | **FIXED** `2534164dd`. `Message::state_result` grew a 34th parameter; both call sites passed 33, so a String slid into an `Option<Value>` slot and rustc blamed a correct line. Now 39/39. |
| **35 remaining `tests/source_audits` failures** | — | Down from 42. The seven fixed were false reds (see the morning update); these 35 are genuine — `footer_safe_scroll_offset_for_item` no longer exists anywhere, the `agent_chat::ui` façade re-exports non-runtime types it forbids, and 14 outer files bypass that façade. Fixing them changes **production code** and decides architecture. |
| Shared worktree, shared git index | — | Two agents committing in one tree produced one misattributed commit (`5ce5b3530`). If lanes keep running in parallel here, they need separate worktrees — this will recur. |

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

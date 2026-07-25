# AI rock-solid UX — Oracle Execute ledger

## Immutable premise

- premise: `.notes/oracle/ai-rock-solid-ux/premise.md`
- premise sha256: `149dc8e3772c0bf47c559694767cdc50694529971ffd4038ea91afaddd16b393`
- plan: `.notes/oracle/ai-rock-solid-ux/plan.md`
- plan steps: 15 execute, 0 think (`S00`–`S14`)
- current status: active; whole premise not yet proven
- ledger last reconciled: 2026-07-24 (had gone stale at 05:06, missing the S08
  commit and all S09/S10 work; user scope decisions added below)

## Consult ledger

| Purpose | Slug | Transport | Routing receipt | Result |
|---|---|---|---|---|
| Plan | `ai-rock-solid-ux` | qualified round-robin browser, Pro Extended | protocol v2; `profile-b-2`; allocation `profile-lease-00000380`; run `run-da2027f2e1a4fd87ab48d3fd24987b3a` | completed; full coverage map and S00–S14 plan |
| S08 escalation | `quick-ai-latency-fix` | qualified round-robin browser, Pro Extended | protocol v2; `profile-a`; allocation `profile-lease-00000397`; run `run-3570e52687f7c8f1b7e04130ca3d4dab` | completed; one native transaction, app deadline, early finalization, and two-batch hard gate |

- consult count: 2
- transport fallback: none
- escalations: S08 after two pre-escalation latency failures
- audit: pending

## Credential handling

- **Source:** runner-managed persistent signed-in headful ChatGPT profile on the qualified remote staging pool; runner token remained in process memory.
- **Scope:** least-privilege ChatGPT planning and S08 escalation analysis only; no production mutation. Repository probes use the separately documented local sandbox credential path below.
- **Redaction:** no token, cookie, authorization header, browser profile data, or raw authenticated response is stored here; only allowlisted routing IDs/meta and returned plan text are retained.

## Process/routing receipts

- `md flows/devtools.md ...`: **Codex isolation preparation failed before engine start; direct fallback authorized.**
- `md flows/agent-chat.md ...`: **Codex isolation preparation failed before engine start; direct fallback authorized.**
- Direct repo-native DevTools intake continued per `AGENTS.md`; do not retry these flow calls during this execution.
- Cargo policy: all Cargo commands use `./scripts/agentic/agent-cargo.sh`.
- Lifecycle: commits are scoped per green Oracle step; no push, release, or deployment is authorized.

## User scope decisions (2026-07-24, supersede plan defaults)

Collected via `web-choices`, submissions `11274034-dafe-4537-b8be-493e9fa76776`
and `492fd4f8-9af8-4146-a7d0-60e810f702b7`. These are the user's own answers and
outrank the plan's implied scope.

| Decision | Answer |
|---|---|
| Flow ↔ Agent Chat parity | **100 — "one conversation experience"** (user overrode the recommended 60; distinctions live in what each surface can DO, not how it behaves) |
| Runtime proof depth | **100 — "film every surface"** (user overrode the recommended 50) |
| S08 disposition | Blocker. Fix all three defects, then pass **several batches in a row**. One green run is not acceptable evidence. |
| S09 scope | **70** — make what exists work AND survive app restart. Excludes run-level recovery. |
| S10 scope | **60** — connect the recovery buttons. Excludes the full ~14-case test matrix. |
| S11 scope | **50** — remove only leftovers that can still reach the screen. |
| S12 scope | **100** — film every surface. |
| S13 scope | **60** — write the rules plus focused checks, not the full suite. |
| S14 scope | **40** — outside review of the risky parts only. |
| Order | **Errors first**, then the everyday consistency pass. They touch the same chat files, so running both at once would conflict. |
| Plan disposition | Keep it, then add the missing half — the plan only covers how surfaces FAIL, not how they behave when they work. |

Two behaviors the user designed directly, rejecting all offered options:

- **Escape**: every AI experience shows a modal offering **Background** or **Close**.
  This also removes the lying `"Esc Desk"` footer, since the modal names the destination.
- **Click-away**: all experiences **auto-background**, and a backgrounded session
  becomes the **first option in the main menu**. This is a feature, not a dismissal
  setting — it is what gives the Escape modal's Background option a visible place to
  land. User chose to **design it fully before building**.

## Step status

| Step | Tag | Status | Commit | Verification receipt |
|---|---|---|---|---|
| S00 Freeze owners/dirty exclusions/red baseline | execute | complete | `4fa7d5d16` | Quick AI 1/1; ChatPrompt types 8/8; Bun benchmark 8/8 |
| S01 Typed app-independent reliability state machine | execute | complete | `c143e3960` | 11 model tests passed; crate check passed; dependency/effect audits clean |
| S02 Failure normalization and diagnostic redaction | execute | complete | `8de3264cf` | unit 6/6; integration 3/3; screenshot categories and redaction matrix green |
| S03 Direct DevTools reliability observability + red receipts | execute | complete | `789da18fe` | Bun 5/5; Rust 10/10; three strict runtime red probes reproduced |
| S04 Capability evidence and truthful selection preflight | execute | complete | `c6dc4d023` | preflight 35/35; models 6/6; contract 6/6; stale profile launch blocked |
| S05 Typed runtime boundaries | execute | complete | `acdce5f6d` | check green; runtime seam 7/7; Codex 19/19; Pi 51/51; Flow model 16/16; runner 4/4; ChatPrompt 55/55 |
| S06 Shared recovery projection/component | execute | complete | `3be4baf2a` | projector 6/6; component 3/3; public integration 2/2; check green |
| S07 Agent Chat/setup recovery migration | execute | complete | `96e7efa95` | protocol 12/12; reliability 16/16; warm 2/2; thread 86/86; view 70/70; auth + retry runtime probes green |
| S08 Quick AI search-budget prevention/recovery | think (escalated after two red attempts) | complete | `52e66fb88`, `fc32a72d2`, `0f42c5930` | see "S08 closure" below: 16/16 answered, 0 recovery cards, 0 protocol failures across 6 queries x 3 reps of real streams |
| S09 Flow conversation/run recovery | execute | complete | `d86e8a679` | flow recovery survives relaunch: `PersistedAiFailure` on the persisted conversation, `FlowChatRequest::Recovery` routing; `flows::` + `prompts::chat` focused suites green |
| S10 Legacy ChatPrompt migration | execute | complete | `d86e8a679` | `with_recovery_callback` now has a caller; `RethreadFlow` added to the flow surface's capabilities; new `recovery_actions_appear_only_when_the_host_can_perform_them` locks the filter |
| S11 Remaining AI integrations/stringly cleanup | execute | complete | `fcfcf4595` | root cause was one defect class, not many sites: classify → reduce to a String → re-classify. `AgentChatWarmSessionSnapshot.failure` is now `Option<AppFailureRecord>`, so the round-trip is unrepresentable. Three new typed classifiers; the 3 warm-session tests red since S05 are green |
| S12 Green runtime/semantic/layout/screenshot receipts | execute | complete | `460d0d99f`, `45bf246fe` | filming found three defects unit tests could not: codex `Unknown` → `ChildExited`, Agent Chat `Unknown` → `RuntimeClosed`, and an **invisible** recovery card that made every on-screen assertion unfalsifiable. `ai_recovery_elements` now projects `recovery_semantic_tree` into the element collector |
| S13 Contract docs and repository-wide gates | execute | complete | `b803dfcdc` | `rules/AI_RELIABILITY.md`: six rules, each naming its real defect, plus focused-check commands. Gotcha recorded: `AGENTS.md` is a symlink to `CLAUDE.md` |
| S14 Whole-premise Oracle audit + identical-state local proof | execute | in progress | — | **Oracle was unreachable this session** (api engine: `Missing OPENAI_API_KEY`; browser engine: `ECONNREFUSED 127.0.0.1:55894`). Substituted an independent-model Codex subagent adversarial review of the five risky changes; awaiting its verdict |

## S08 closure (2026-07-24)

The earlier red gate was measured against hand-written events and a synthetic
query. Re-running it against REAL `codex exec --json` streams found six defects
that the synthetic harness could not see, and the gate is green once they are
fixed. Harness: `scripts/agentic/quick-ai-codex-stream-corpus.ts`, which reads
the model, prompt, schema and the literal `.arg(...)` sequence out of
`build_codex_exec_command` so it cannot drift from production.

Fixed in `fc32a72d2` and `0f42c5930`:

1. Quick AI could not answer a question that needed no web search — the
   provenance gate required `search_completed` before any answer passed.
2. The fast path (`CompleteEarly`) skipped every source check.
3. Source verification was unreachable: a `search` action carries no result
   URLs, so `structured_urls` is always empty and the host check never ran.
4. Codex still offered a shell tool and used it — a captured stream shows
   `/bin/zsh -lc 'recall context'` reading the user's shared agent memory
   during a Quick AI turn.
5. Provider stderr, not our own code, decided the failure kind, so a blocked
   shell command told the user to sign in.
6. A red prompt test had been hidden since `52e66fb88` because the S08 gate
   filter (`codex_quick_ai`) does not match its path.

Measured after the fixes — 6 queries x 3 reps, real streams:

| Question | Answer |
| --- | --- |
| Did the one-search policy hold? | Yes. 17/17 runs: at most one `web_search` item, zero page visits. The production system prompt is what enforces it; without the prompt the model searches AND opens pages, and 6/6 web-requiring queries ended in a recovery card. |
| Are the batches green? | 16/16 answered, 0 recovery cards, 0 protocol failures. Re-score saved streams with `--classify-only`. |
| Is the 11,650ms deadline right? | Yes. 1/17 runs exceeded it (33.7s, a stalled search); the rest finished at 2.3-9.1s. That tail already degrades gracefully — `codex_quick_ai_deadline_preserves_partial_and_reaps_process` keeps the partial answer and reaps the process. No change made. |

Residual limitation, stated rather than hidden: 10/16 answers cite a host no
page visit backs. Snippets-only searching makes real verification impossible
without a page fetch, and a fetch does not fit the latency budget. The code no
longer claims otherwise — the trace label is now
`unvisited-validated-schema-source`, and the enforced guarantee is that every
URL shown passed schema validation and followed a completed search.

## S00 working receipt

- product source edits in this execution before S00: none
- pre-existing dirty exclusion snapshot: `.notes/oracle/ai-rock-solid-ux/preexisting-dirty-paths.txt`
- integration owner inventory: `.notes/oracle/ai-rock-solid-ux/integration-inventory.md`
- initial screenshot/source intake: `.notes/oracle/ai-rock-solid-ux/devtools-intake.md`
- baseline checks: `.notes/oracle/ai-rock-solid-ux/s00-baseline.log`
  - Quick AI third-search fail-closed fixture: 1 passed, 0 failed.
  - ChatPrompt type tests: 8 passed, 0 failed.
  - Quick AI web-search benchmark contracts: 8 passed, 0 failed.
- S00 product-source changes: none; only the three S00 note artifacts are eligible for the scoped commit.

## S01 working receipt

- Added the pure `sk_protocol::ai_reliability` typed domain and reducer.
- States/events/commands/outcomes/failure categories/recovery actions use exhaustive enums.
- Effects are emitted only as typed commands; the crate has no app, GPUI, filesystem, process, clock, randomness, or async-runtime dependency.
- Model coverage includes deterministic Cartesian state/event decisions and bounded generated three-event sequences.
- Final verification: `.notes/oracle/ai-rock-solid-ux/s01-final-verification.log`.
  - `./scripts/agentic/agent-cargo.sh test -p sk-protocol ai_reliability -- --nocapture`: 11 passed, 0 failed.
  - `./scripts/agentic/agent-cargo.sh check -p sk-protocol`: passed.
  - dependency tree: only `serde` and its derive stack; no reverse app dependency.
  - wildcard reducer arms: 0; app/effect dependencies: 0.
- Mechanical S01 plan amendment: include the one-line `sk-protocol -> serde` root `Cargo.lock` dependency projection required by the crate manifest.

## S04 working receipt

- Added bounded fingerprint-keyed capability evidence; submission snapshots spawn zero processes.
- Exact negative client/model evidence blocks before turn start; unknown evidence stays truthful and permits only the single normal runtime attempt.
- Persisted stale Agent Chat and Quick AI profile selections now block actual launch with `ai_profile_selection_unavailable:<id>` instead of silently switching to Brain/built-in Quick AI.
- Runtime `ModelsAvailable` preserves valid user selection; missing/empty model catalogs produce typed recovery state and never assign runtime `current_model_id` as user intent.
- Final verification: preflight 35 passed; models 6 passed; integration contract 6 passed; focused stale-profile launch test 1 passed; formatting and exclusion hash audit green.

## S05 working receipt

- Replaced stringly Agent Chat and Flow terminal events with typed completion, cancellation, and `AppFailureRecord` outcomes.
- Made the object-safe Agent Chat connection seam return typed immediate adapter failures.
- Classified Quick AI, Pi, Flow, and legacy ChatPrompt runtime failures at their owning boundary; primary UI receives safe copy while raw diagnostics stop at the redacting vault.
- Raw Codex/mdflow stderr is redacted or reduced to stable code and fingerprint before logs, traces, or retained tails.
- Final verification: library check passed; runtime seam 7/7; Codex runtime 19/19; Pi 51/51; Flow model 16/16; Flow runner 4/4; Flow client 4/4; registry 18/18; ChatPrompt 55/55; reliability 8/8; obsolete terminal-string grep zero; formatting and exclusion hash audit green.

## S06 working receipt

- Added a pure, exhaustive recovery projector covering every AI surface identity, phase, and failure category.
- Added a shared tokenized recovery card with four layouts, stable semantic IDs, one-primary/two-secondary action bounds, subordinate diagnostics, preservation/progress/recovered states, and no motion constants.
- Added an independently testable keyboard decision model for Tab, Shift+Tab, Enter, Space, and Escape plus a public behavior-contract target.
- Final verification: projector 6 passed; component 3 passed; public integration 2 passed; library check passed; formatting and exclusion hash audit green.

## S07 working receipt

- Made each `AgentChatThread` the single owner of an `AiOperationState`; submit, preflight, runtime, failure, retry, dismiss, cancellation, selection, and session replacement now transition through the typed reducer.
- Replaced the legacy callout/auth/warm failure shapes with the shared recovery projector and semantic card IDs. The exact old-Codex/model incompatibility now presents safe update/model actions without exposing raw provider JSON.
- External sign-in and update launches stay in actionable recovery until capability health is observed; retry resumes the preserved turn without duplicating its user row.
- Added a live, redacted DevTools projection of the thread-owned state. Explicit fixtures can still overlay red scenarios, but prompt state no longer overwrites real Agent Chat reliability with a detached ready default.
- Final verification:
  - `sk-protocol` reliability model: 12 passed.
  - app reliability, warm recovery, Agent Chat thread, and Agent Chat view suites: 16, 2, 86, and 70 passed respectively.
  - `SCRIPT_KIT_AGENT_ARTIFACT_NAME=agent-chat-ai-recovery ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui`: passed; artifact exported.
  - auth runtime probe: `awaitingRecovery`, `UsageExhausted`, primary `ai-recovery-switch-account`, raw-primary hidden; screenshot inspected.
  - retry runtime probe: failed turn `[user-1,error-2]` became `[user-1,error-2,assistant-3]`, final phase `succeeded`; no duplicated user row; screenshot inspected.
  - obsolete recovery-seam grep: zero; staged/exclusion intersection: zero; staged diff check and hook formatting gate: passed.
- Concurrent glass, Notes, footer, platform, vendor, and DevTools edits remained unstaged and untouched; the original dirty-path hashes changed in the separately owned lane, so no whole-tree exclusion-hash-green claim is made.

## S08 escalation receipt

- Escalation output and routing receipts:
  - `.notes/oracle/quick-ai-latency-fix/oracle-output.log`;
  - `.notes/oracle/quick-ai-latency-fix/oracle-readable.md`;
  - `.notes/oracle/quick-ai-latency-fix/oracle-meta.json`.
- Implemented the escalated contract:
  - one app-admitted native web transaction means one provider item plus one normalized non-URL query; same-item lifecycle updates remain one action, while a second item, changed/multiple query, or page follow stops before admission;
  - app-owned `11,650ms` work deadline plus `350ms` teardown reserve;
  - early finalization only after one admitted search completes and the completed agent message passes the strict app validator;
  - Codex structured-output schema uses the provider-supported strict subset while app validation enforces non-empty/max-1,200 answer, max-three unique canonical HTTP sources;
  - app-owned redacted trace reports ordinals/timings only; no raw item ID, query, action body, tool name, auth body, header, token, or cookie;
  - sandbox driver now pins `CODEX_HOME` under its fresh HOME instead of inheriting the calling agent's home.
- The typed search-budget path is implemented and its deterministic runtime probe is green:
  - surface `quickAi`, phase `awaitingRecovery`, failure `QuickAiSearchBudgetExceeded`;
  - primary `ai-recovery-continue-agent-chat`, secondary `ai-recovery-use-current-results`, no Retry;
  - real partial answer and safe source retained; raw diagnostic hidden;
  - primary action opens a fresh standard Agent Chat composer seeded with the original question and sources;
  - budget receipt `.test-output/ai-rock-solid-ux/quick-ai-policy-budget.json`, sha256 `cce46d5cb22a4b6c7fb91e457f5a3caa7afb8abe8369a530f29b7e881f6364a5`;
  - deadline receipt `.test-output/ai-rock-solid-ux/quick-ai-policy-deadline.json`, sha256 `f6be3dc04bc2c9b023e848b723c65e62d9f1b3a6ec78fb1390674c54df0abbec`;
  - screenshots `.test-screenshots/ai-rock-solid-ux/quick-ai-policy-budget.png` and `.test-screenshots/ai-rock-solid-ux/quick-ai-policy-deadline.png`, both visually inspected.
- Focused correctness gates are green after escalation: protocol 14/14, Quick AI 33/33, reliability 17/17, Bun benchmark/probe/driver 18/18, exported binary build, deterministic budget/deadline runtime probes, and diff checks.
- Required fastest-search runtime gate failed on two separate attempts:
  - `.test-output/ai-rock-solid-ux/quick-ai-fast.json`: 3/3 valid, zero-context/source/orphan gates green, median `16,571ms` against `12,000ms`.
  - `.test-output/ai-rock-solid-ux/quick-ai-fast-rerun.json`: 2/3 valid, one transient `AuthenticationMissing`, median `33,954ms`; valid trials were `18,510ms` and `33,954ms`.
  - multi-action trials emitted 3–4 native search action events even though the profile contract requests exactly one focused search; the current budget guard counts distinct focused searches and did not project policy recovery for those same-focus action sequences.
- Pre-escalation failures remain preserved. The first post-escalation real run also exposed and then repaired two probe/product defects without weakening the product gate:
  - inherited runner `CODEX_HOME` bypassed the sandbox seeder; preserved at `.test-output/ai-rock-solid-ux/quick-ai-fast-auth-injection-failure.json`;
  - unsupported JSON Schema keywords caused provider rejection that broad text heuristics mislabeled as authentication; preserved at `.test-output/ai-rock-solid-ux/quick-ai-fast-schema-failure.json`; the live schema is now provider-supported and schema rejection classifies as configuration.
- Post-escalation hard gate:
  - Batch A `.test-output/ai-rock-solid-ux/quick-ai-fast-a.json`, sha256 `507d728e442e46dd3ede04c7feec56d9394111062099f5f20b1cea8d4fd47d18`: **PASS**, 3/3 valid, median `9,679ms`, max `11,329ms`, one permit/action per trial, source proof, zero context, no page follow/Pi/raw identifiers, orphan-free; cancellation `158.19ms`.
  - Independent batch B `.test-output/ai-rock-solid-ux/quick-ai-fast-b.json`, sha256 `319e99c4ce5f95184de821a3e671c75a242f84c4944ab4b39a41972a453fabd0`: **FAIL**, 0/3 valid. Trial 1 truthfully reached deadline at `11,675ms` before an answer/source. Trial 2 emitted a forbidden provider item and failed closed. Trial 3 completed in `9,934ms` but supplied unverified `https://rust.dev/updates`, so official-source proof correctly failed. Cancellation remained green at `155.16ms`; all trials were zero-context, no-Pi, and orphan-free.
- Oracle's explicit stop condition now applies: retain the red receipt, do not relax the threshold, change the query, omit invalid trials, or tune unboundedly. S08 has no commit and S09 must not start while this hard gate is red.
- Additional optional binary-test compile check exposed four test-only compile errors outside the S08 runtime claim (`InboxKind` path and three stale `anyhow::Result` trait implementations). The required library build/check and exported binary build remain green; these errors are retained for later repository-wide closure rather than hidden.

## App-probe credential handling

- **Source:** the repository's sandbox-home seeder injected the already-authorized local Codex session through its non-interactive `auth.json` path; no literal credential was passed.
- **Scope:** least-privilege OpenAI Codex inference and public web search for the isolated Quick AI benchmark sandbox only; no production mutation.
- **Redaction:** tokens, authorization headers, cookies, and credential contents were not printed or retained; receipts keep only allowlisted run/timing/process/source metadata and public test answers.

## Post-S08 working receipt (2026-07-24, S09-S14)

Commits, in order: `d86e8a679` (S09+S10), `fcfcf4595` (S11), `460d0d99f` and
`45bf246fe` (S12), `b803dfcdc` (S13), `e7850decd` (backgrounded-sessions design).

The single most useful finding across these five steps: **S11's "leftover raw
error paths" and the separate "Agent Chat auth falls through to Unknown" bug
were the same defect.** A failure was classified correctly, reduced to its
user-facing string, and re-classified from that string. Safe copy carries no
provider evidence by design, so the second pass always returns `Unknown` — and
the user lost the Sign In button on the one failure a Sign In button fixes.
The fix is structural, not a patch: the carrier field is now
`failure: Option<AppFailureRecord>`, so there is no field to put a string in.

S12's runtime filming found three defects the unit suites could not reach,
including one that invalidated its own proof: the recovery card was invisible
to `getElements`, so every "the card is on screen" assertion passed whether the
card rendered or not. `collect_visible_elements` is a hand-written surface
model, not a GPUI tree walk — anything absent from it is unfalsifiable.

Preserved distinction under pressure: when pi's stderr says WHY it died, that
evidence still wins. Only the evidence-free case degrades to `RuntimeClosed`,
so an auth death keeps its Sign In action.

Pre-existing failures catalogued, not hidden: `window_state_audit` (x2),
`actions_button_visibility_tests`, `components::error_handling_audit_tests`,
`dictation::tests` (x2), and
`flows::session::tests::of38::turn_arg_resolution_uses_one_deadline_for_both_shapes`
(passes 1/1 in isolation with `--test-threads=1`; flaky only under parallel load).

## S14 boundary (stated, not worked around)

Oracle could not be consulted this session. The `api` engine failed with
`Missing OPENAI_API_KEY` and the `browser` engine with
`ECONNREFUSED 127.0.0.1:55894`. Rather than skip the step or claim it green, an
independent-model Codex subagent reviewed `/tmp/sk-risky.diff` against
`rules/AI_RELIABILITY.md` on five points: the reducer's `manual_retry_option`
risk-gate removal; the pi `read_stdout` stderr-hint race; warm_session
information loss; `codex_client` `try_wait` reap safety against
respawn/generation; and the three new classifiers. That is a weaker instrument
than the planned whole-premise Oracle audit and the step should be re-run when
Oracle is reachable.

## S14 findings (2026-07-24)

Reviewed the five risky changes in `/tmp/sk-risky.diff` against
`rules/AI_RELIABILITY.md`. One confirmed defect, fixed in `03c2cdf7e`.

| # | Point | Verdict |
|---|---|---|
| 1 | reducer `manual_retry_option` drops `risk`/`progress` | **No defect — it is the fix.** Every Agent Chat and Flow turn is `TurnRisk::MayMutate` (`thread.rs:748`, `session.rs:587`), so the old gate rendered the primary Retry disabled (`UnsafeToReplay`) on essentially every real failure. Residual gap recorded below. |
| 2 | pi `read_stdout` stderr-hint race | **CONFIRMED DEFECT, fixed.** stdout EOF and the stderr line that explains it come from two independent readers; losing the race replaced the Sign In card with a Reconnect card. `await_stderr_hint` now waits up to 250ms, polling every 10ms, only when a hint slot exists and is empty. |
| 3 | warm_session information loss | **No defect.** The record is stored on the slot before `anyhow::bail!` returns safe copy; consumers (`agent_chat_launch.rs:891`, `:911`) read `snapshot.failure`, not the bailed string. Narrow edge noted below. |
| 4 | `codex_client` `try_wait` reap vs respawn/generation | **No defect.** The `take()` + `try_wait()` pair is byte-for-byte the pre-S12 behavior; only the fact extraction is new. The whole EOF block runs under the `child_generation` guard, and `ChildExited { None, None }` classifies as `ChildExited`, never `Unknown`. |
| 5 | the three new classifiers | **No defect.** All route through one `record()` helper: cause to the vault, kind from the fact, `retry_safety` derived from the kind. `Runtime(_)` maps to `ExplicitUserConfirmation`, so their cards keep an enabled Retry. |

Two gaps recorded rather than fixed (neither is reachable today):

- **The half-mutated-turn guard is inert on both ends.** `mutating_effect_started`
  is hard-coded `false` at its only producer (`thread.rs:3384`) and, since the
  reducer change, is read by nobody in the manual path. When Agent Chat learns to
  report that a turn started writing files, manual Retry must consult it again —
  otherwise one keypress replays a half-applied mutating turn.
- **Warm spawn generation edge.** In `warm_session.rs`, a spawn failure whose slot
  generation has already moved on drops the typed record and still bails with safe
  copy. No current caller re-classifies that string, so it is latent, not live.

Boundary, stated plainly: this review was performed in-session against the
source, not by the planned outside Oracle audit. Oracle was unreachable (`api`:
`Missing OPENAI_API_KEY`; `browser`: `ECONNREFUSED 127.0.0.1:55894`), and a
delegated Codex subagent returned nothing. S14 should be re-run when Oracle is
reachable; the five points above are the exact scope to hand it.

Correction (same day): the delegated reviewer did not "return nothing" — the
`s14-review` subagent failed three times with *"There's an issue with the
selected model (gpt-5.6-sol). It may not exist or you may not have access to
it."* That is a configuration failure, not a silent one, and it is worth
recording because 29 of 29 flow files pin that model.

RETRACTED same day: the inference above was WRONG. A direct check —
`codex exec --model gpt-5.6-sol "reply with exactly: MODEL_OK"` — returns
`MODEL_OK`. The model is available to codex and every flow is fine. The failure
was Claude Code's own subagent model resolution ("Run /model to pick a
different model" is Claude Code's string, not codex's), so it says nothing
about mdflow. Lesson, and it is the same one this whole workstream is about:
an error message was read as evidence for a system it never described. The
one-command check that settles it costs nothing; the inference cost a wrong
line in a commit message (`3ccdfc5d9`).

## Everyday parity pass (flows disabled, worked in-session)

Four commits, each with its own falsifiable check. The through-line: every
defect here was **silent** — the surface kept working and looked right, so
nothing prompted the user to doubt it.

| # | What | Proof | Commit |
|---|---|---|---|
| 22 | Pasted newlines deleted, welding words | probe red → green at runtime | `32aac4798` |
| 17 | `⌘.` Stop precedence was statement-order luck | `agent_chat_cmd_period_stops_streaming_before_reopening_a_mention` | `32aac4798` |
| 18 | Footers advertised different Stop chords | `agent_chat_and_flow_advertise_the_same_stop_chord` | `9028268cf` |
| 23a | Flow had no Copy Last Response | `flow_sessions_copy_the_last_response_the_same_way_agent_chat_does` | `9028268cf` |
| 22b | Same newline defect on the set-text path | `single_line_normalization_tests` 5/5 | `b2fc53747` |
| 23b | Up-arrow prompt history dead in Flow | 4 tests incl. an all-combinations range sweep | pending |

### What the runtime receipt actually bought

The paste corruption was already proven in source at three layers. The probe
still earned its keep twice:

1. It made the claim falsifiable in both directions — it records the exact
   composer string and classifies it, rather than asserting an outcome.
2. Re-running it after the fix caught that the *fix itself* changed what
   "correct" looks like. The probe would have reported red on a working build
   because `"Fix the bug in auth.rs"` matched neither the pasted text nor the
   known-corrupt form. A probe that only knows one failure mode reports the
   repair as a new bug.

### The defect class, stated once

Three separate bugs this pass reduce to the same shape: **a value is degraded,
the degraded value still looks plausible, and the code that would have warned
never runs.** Newlines deleted before the app's newline guard could see them.
A Stop chord whose winner depended on which `if` came first. A footer honestly
describing a surface, teaching a rule that broke on the next one.

None of these fail loudly. All of them need either an invariant that cannot be
satisfied by accident, or a probe that reads the value back.

### Boundaries

- Escape still means Stop in Agent Chat and Background in Flow. Deliberate —
  that belongs to the Escape modal design, not to a footer-label change.
- Flow still cannot hold a multi-line message or honor Shift+Enter. The
  corruption is fixed; line *structure* needs a dedicated Flow composer, since
  the shared input is also ScriptList's, where Enter must keep meaning
  "select". The probe reports this as `structureLossStillOpen` so a flattened
  run cannot read as complete.
- Still open: shared `ConversationStyle` (two different markdown engines) and
  the composer unification. Both large, both need a design decision rather
  than a patch.

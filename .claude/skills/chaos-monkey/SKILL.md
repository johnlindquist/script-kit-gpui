---
name: chaos-monkey
description: >-
  Run the ultimate chaos-monkey campaign against script-kit-gpui: walk every
  user scenario and story across every surface while capturing correctness,
  perf, layout/CLS, data-integrity, and design-vision drift; triage each
  finding, fix it, lock it with a test or probe; iterate on a durable scenario
  ledger until the closure gate passes and two consecutive full passes find
  nothing new. Use when asked to chaos-monkey, stress, exploratory-QA, or
  vision-audit the app, or to verify the app meets its design goals end to end.
---

# Chaos Monkey — the full campaign

Every user story in this app must earn **three verdicts**, not one:

1. **It survives.** Hostile-but-plausible reality (bad input, corrupt state,
   churn, races, encoding edges) never produces a crash, hang, torn state,
   silent wrong answer, or injection.
2. **It performs.** Keystroke→render latency, frame stability, and draw share
   stay inside budget under the story's real load — with numbers, not vibes.
3. **It matches the vision.** The rendered result obeys `.impeccable.md`'s
   principles and the design-contract tokens — not merely "works."

Classic chaos testing finds crashes but walks no stories. Story walkthroughs
find UX gaps but only on happy paths. This campaign multiplies them:
**every story × chaos classes × capture lenses**, tracked in a durable ledger,
iterated until closed. "Done" is a closure-gate calculation plus two
consecutive clean full passes — never fatigue.

This skill is the orchestration contract. The breakage doctrine (prime
directives, sandbox rules, batteries A–I) lives in `.notes/chaos-monkey.md` —
read it first and treat its §0 prime directives as binding here too. The
devtools primitives live in the `script-kit-devtools` skill. Perf physics and
the draw-share method live in `flows/perf.md`. Reuse all three; do not restate
or reinvent them.

---

## Standing constraints (violating these voids the run)

- **All cargo through `./scripts/agentic/agent-cargo.sh`.** Never bare
  `cargo` — `./dev.sh` holds the shared target lock.
- **Sandbox everything.** `Driver.launch({ sandboxHome: true, ... })` for every
  probe; never fuzz the real `~/.scriptkit`/`~/.kit`/clipboard store. Seed auth
  only when a story needs live Agent Chat (`seedAgentAuth: true`).
- **No destructive probes.** Script-execution stories run against inert stubs
  (`/bin/echo`-style) — never a real destructive command. No synthesized global
  input on the live desktop; native escalation only inside controlled probe
  windows, and only when the OS layer is what's under test.
- **Fixture files via the file tool**, never shell heredocs/`printf`
  (multibyte/quoting mangling).
- **Environment ≠ bug.** Missing pi sidecar, unauth LLM, denied TCC
  permission, machine load, stale binary, your own harness mistake — classify
  as environment, never inflate the count.
- **Fix, then lock.** Every accepted bug/design-drift fix gets a
  fails-before/passes-after lock at the highest rung of the enforcement ladder
  that can express it (see Phase 4).

---

## Phase 0 — Preflight (once per campaign)

```bash
# 1. Stable binary the whole campaign pins to (APFS clone, atomic on rebuild):
SCRIPT_KIT_AGENT_ARTIFACT_NAME=chaos ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui
export SCRIPT_KIT_GPUI_BINARY=target-agent/artifacts/chaos/script-kit-gpui

# 2. Pi sidecar (Agent Chat / brain stories show "unavailable" without it):
bash scripts/agentic/ensure-pi-sidecar.sh

# 3. Baseline must be green BEFORE fuzzing (and after every fix):
./scripts/agentic/agent-cargo.sh test 2>&1 | tail -8
./scripts/agentic/agent-cargo.sh clippy --all-targets --all-features 2>&1 | tail -8
cargo fmt --check 2>&1 | tail -3
```

Confirm the driver's stderr `[driver] binary:` line names YOUR artifact on the
first launch — the freshest-by-mtime fallback is a stale-binary trap.

Sandboxed (seatbelt) agents cannot launch the GUI: every RPC times out →
classify `blocked-by-sandbox`, have the caller run
`bash scripts/agentic/session.sh start <name>` outside the sandbox, and use
`Driver.attach({ session })`.

---

## Phase 1 — Story harvest → the ledger

Build (or refresh) one durable ledger before executing anything:
**`.notes/chaos-ledger.md`**, in the `scenario-matrix-closure` shape — rows
with `row_id`, dimension values, expected outcome, executor contract, status
(`planned/ready/running/blocked/observed/verified/failed/waived`), append-only
attempts with evidence refs. Reread it before every status transition; never
close rows from conversation memory.

**Row dimensions:** `surface × story × chaos-class × lens`.

Harvest stories from these sources, in order — they already exist, do not
invent from scratch:

| Source | What it gives |
|---|---|
| `GLOSSARY.md` | The surface inventory (launcher, prompts, built-ins, notes, dictation, brain/day-page, flow desk, actions, confirm, terminal…) — every surface gets rows |
| `.impeccable.md` | The 7 design principles + token tiers — each principle becomes a testable claim per surface (footer ≤3 keys; discovery in ⌘K; peek not clutter; whisper-chrome opacities; sub-frame input; keyboard-reachable; native vibrancy) |
| `docs/guides/feature-tour.md`, `getting-started.md` | Narrative first-run and daily-driver journeys |
| `.notes/user-stories.md` | ACP-S1…S10 sigil stories, PERF-1…6, UX-1…8 (with known protocol gaps) |
| `.notes/power-user-stories.md` | The grammar acceptance matrix (`+ : # ! / @` sigils; plain text stays fuzzy search) |
| `scripts/hitl-choice/data/script-kit-qa-scenarios.json` + `.hitl-choice/script-kit-qa-scenarios-v2-25.json` | ~75 structured QA scenarios with per-scenario proof commands and pass/fail evidence |
| `.claude/skills/script-kit-devtools/references/devtools-truth-scenarios/` | ~40 executed truth-scenario receipts — treat as covered; don't re-prove, chaos around their edges |
| `references/surface-contract-matrix.md` (this skill) | The affordance-parity oracle: per-surface expected footer affordances + overlay symmetry contract + ratified divergences — every surface gets a parity row |
| `tests/*_contract.rs` corpus, `docs/adr/` | Already-locked invariants and decisions — the "intent" side of reality-vs-intent |

**Chaos classes** (from `.notes/chaos-monkey.md` batteries, applied *per
story*, not just globally): nominal · empty/zero-results · hostile input
(encoding edges, huge, control chars, pathological filters) · corrupt/churning
state · rapid interaction (open-while-opening, Esc storms, hold-repeat) ·
**chord/affordance symmetry** (every chord that opens an overlay must toggle it
closed on repeat, close on Escape, restore focus to the pre-open owner, and
leave the underlying footer/chrome intact — run surface × registered chord) ·
degraded environment (LLM down, permission denied — expect graceful clarity) ·
recovery (cancel mid-stream, relaunch dirty).

**Lenses** (what gets captured — Phase 2): correctness · perf · layout/CLS ·
data-integrity · design-vision · affordance-parity.

**Finite selection, not Cartesian:** every surface gets at least a nominal
story, an empty state, one hostile-input row, one perf row, and one layout row.
Add pairwise/higher-order rows only for risk (prior incidents, changed
boundaries, sigil interactions, escape-ladder nesting). Record exclusions and
why. When code affecting a row changes, append `attempt_invalidated` and
return it to `ready` — proof goes stale, the ledger says so.

---

## Phase 2 — Execute rows: executors and capture lenses

**Executor ladder** (pick the lowest that can observe the row):

1. **Driver protocol probe** (default; ~10–50ms/step; parallelizable;
   hidden-window — no `show` needed): `scripts/devtools/driver.ts` —
   `getState`, `getElements`, `getLayoutInfo`, `setFilterAndWait`, `batch`,
   `simulateGpuiEvent` (the only path that arms @mentions), `waitForSettle`
   (never hardcoded sleeps), `getLogs`, `captureScreenshot`, fixtures
   (`openAgentChatKitchenSinkFixture`, `pushDictationResult`, …).
2. **Fail-closed receipt CLIs** (`scripts/devtools/*.ts`: `surface.ts`,
   `layout.ts`, `scroll.ts`, `focus.ts`, `actions.ts`, `compare.ts redgreen`,
   `events.ts crashes`) when a row needs target-identity receipts or red/green
   deltas for the report.
3. **Native escalation** — only when OS delivery/focus/pointer/scroll IS the
   row: `scripts/agentic/macos-input.ts` (fail-closed evidence), compiled
   CGEvent scroll helper (`PROBE_SCROLL_HELPER` pattern — neither
   `simulateGpuiEvent` nor cliclick can scroll).
4. **`/usr/bin/sample <pid> <secs> -file out.txt` while driving input** — for
   perf rows that need attribution (sampling idle windows proves nothing).
5. **Glancing lane** (one serialized screen-level pass per epoch): walk the
   user journeys frontmost, screenshot each stop, and have a multimodal judge
   answer one question per frame: "what would a user complain about here,
   compared to the main menu?" No metrics, pure judgment, filed as candidate
   findings for triage. Cheap, and it is the lane that finds
   obvious-to-a-human bugs (all five 2026-07 user-reported bugs were visible
   in one glance; no metric probe filed any of them).

**Capture per lens** (a row may capture several):

- **Correctness:** expected view/state via `getState`/`getElements`;
  app-alive; `getLogs` shows **no NEW error entries** relative to the row's
  start (the `chaos-smoke-sheet.ts` PASS/SUSPECT/FAIL pattern);
  `events.ts crashes` clean.
- **Perf:** per-keystroke latency p50/p95 (`root-typing-lag-benchmark.ts`
  pattern); the app's own `PERF`-target log lines
  (`getLogs({target:"PERF"})` → `Search '…' took Xms` —
  `chaos-perf-attribute.ts` pattern); draw-share from a sample file per
  `flows/perf.md` (draw subtree ticks / main-thread ticks, raw counts AND
  ratio). Every perf row states its budget number; red/green is the same probe
  before/after.
- **Layout/CLS:** `getLayoutInfo` paint measurements — real
  `visible_bounds`/`clip_bounds` + `measurement_frame_generation`
  (`src/app_layout/paint_measurements.rs`). Stable chrome (input/footer/header)
  must not drift beyond CLS epsilon across keystrokes/result injection
  (`chaos-cls-perf-probe.ts`, `root-search-frame-stability.ts` patterns);
  `scroll.ts` for occlusion/overflow; extreme window sizes must reflow, never
  interleave garbage.
- **Data-integrity:** snapshot the sandbox HOME's state files before/after;
  assert no torn/partial writes (atomic temp+rename), survival of
  malformed/dir-as-file/huge/concurrent-write corruption
  (`chaos-corrupt-state.ts`), live filesystem churn under the browse path
  (`chaos-dir-browse-churn.ts`), and protocol-fuzz resilience
  (`chaos-protocol-fuzz.ts`).
- **Design-vision:** judge against `.impeccable.md` by principle ID, and
  against the design contract mechanically: export
  `design/mockups/generated/tokens.json` (the `export_design_tokens` bin /
  `src/design_contract/mod.rs`) — the `conflicts` array (two code paths
  disagreeing on one visual value) must not grow; resolved tokens must match
  the documented opacity/interaction tiers. Footer shows ≤3 affordances;
  discovery lives in ⌘K; focused row anatomy (gold bar, tier opacities) holds;
  no new hardcoded visual values (pair with a read-only `flows/auditor.md`
  sweep). Screenshots are evidence for a named principle, never free-floating
  vibes.
- **Affordance-parity:** diff each surface's ACTUAL affordances (footer
  buttons enumerated via `getElements`/the automation surface; registered
  chords) against `references/surface-contract-matrix.md`. Absences and
  asymmetries are findings — a missing button fails no metric, so it must
  fail the matrix. Any divergence not listed as ratified in the matrix is a
  finding regardless of how deliberate the code comment sounds (see the
  divergence-ratification rule in Phase 3). This lens exists because metric
  probes are structurally blind to what SHOULD exist: the 2026-07 campaign
  shipped a Quick Terminal footer whose code said "Actions intentionally
  omitted" and no probe could disagree (OF-60).

**Existing chaos batteries — reuse, then extend** (`scripts/agentic/chaos-*.ts`):
smoke-sheet (9 story smokes) · interaction-stress · corrupt-state ·
protocol-fuzz · encoding-edges · huge-input-latency · perf-attribute /
perf-factors / perf-busy · multisurface-perf · cls-perf-probe ·
dir-browse-churn. New probes follow `<surface>-<behavior>-probe.ts` naming and
become permanent regression gates.

**Parallelization:** hidden-window protocol rows fan out freely — each Driver
launch gets a unique session dir, always with `sandboxHome: true`. Legacy
`session.sh` sessions are name-addressed: loop-unique names or they clobber
each other. **Screen-level rows do not parallelize** (one frontmost window) —
serialize any `show`/screenshot/focus/native-input rows at the end. Per-loop
binaries are `SCRIPT_KIT_AGENT_ARTIFACT_NAME=<loop>` artifact clones — never
per-loop cargo pools (disk budget will evict them).

---

## Phase 3 — Triage rubric

Rank every surprising observation:

- **Bug** — wrong output, hang, panic-without-context, corrupted state,
  swallowed input, injection/path escape, UI garbage, budget blown. → fix.
- **Design-drift** — works, but violates a named `.impeccable.md` principle,
  a token tier, or the shared-component contract (one-off UI, hardcoded
  values, growing `conflicts`, >3 footer keys, hover-dependent affordance).
  → fix through tokens/shared components — never one-off patches; cite the
  principle ID in the fix.
- **Papercut** — correct but confusing/inconsistent. → fix the worst; note
  the rest.
- **By-design** — surprising but defensible and consistent with
  docs/ADRs/contract tests (verify the spawn is argv-vector before calling
  injection "by design"). → document, don't "fix."

**Divergence-ratification rule:** any change (or discovered code) that opts a
surface out of a cross-surface contract — comments like "intentionally
omitted", "deliberately different", scoped-down affordances — must be posted
to the campaign's ratification board AND recorded in
`references/surface-contract-matrix.md` as ratified-or-pending. A deliberate
divergence without a ratification entry is a finding, not a decision.
Grep-enforceable at harvest time: `rg -i "intentional(ly)? omit|deliberate(ly)?"`
over changed files.
- **Environment** — see standing constraints. → name it plainly; not a bug.

For any GUI-runtime bug, run the `flows/devtools.md` loop (intake → primitive
stack → measurements → classification → likely owner → red/green plan) and
attach its receipts to the ledger row.

---

## Phase 4 — Fix & lock

**Fix embargo — discovery and fixing are separate epochs.** Never staff a fix
the moment a red appears: discovery lanes run to their tranche boundary,
findings accumulate on the ranked board, and fixes happen in batched waves
between epochs (the 2026-07 retrospective measured ~60-65% of campaign effort
going to fix-acceptance overhead while discovery starved; the five clearest
bugs arrived from the user, not the herd). Per epoch: cap concurrent fixers,
rank user-visible/affordance findings above metric drift, and end each fix
wave with a frozen-tree re-validation of only the affected rows. The only
exception is a finding that blocks discovery itself (harness/fixture defects)
— those fix immediately, in the harness, not the product.

1. Reproduce minimally; capture the **red** receipt with the exact probe stack.
2. Fix in `src/` or the owning `crates/sk-*` (context-rich errors at
   boundaries; shared components/tokens for UI; domain crates never depend on
   the app).
3. Lock at the **highest enforcement-ladder rung** that can express it:
   compiler/type → clippy lint → behavior test (`#[gpui::test]` /
   `TestAppContext` / unit beside the code, off the GPUI link where possible)
   → runtime probe under `scripts/agentic/` → source audit (last resort;
   follow the source-audit policy — no new count assertions).
4. **Green** = the SAME probe stack that failed now passes — never "a recipe
   ran." Use `compare.ts redgreen` where receipts exist.
5. Gate through the wrapper: `agent-cargo.sh test -p <crate>` (fast) then full
   `test` + `clippy` before declaring a battery clean; `cargo fmt`.
6. Append the attempt + evidence to the ledger row; a fix to shared code
   invalidates every row it could affect.

---

## Phase 5 — Convergence, closure, report

Compute the aggregate ledger state each round (precedence:
`invalid → failed → open → blocked → closed`). The campaign ends only when:

1. the ledger is **closed** — every required row verified/validated/waived for
   the current revision, no material dimension unrepresented, all waivers
   user-approved (agents never approve their own waivers), AND
2. **two consecutive full passes surfaced zero new product findings** — name
   the clean passes.

A happy path, a passing smoke, or "most rows green" is never closure.

**Report** (append to `.notes/`, numbered `chaos-NN-<slug>.md`, matching the
existing convention): per finding — setup → expectation → observed → verdict →
fix + locking test. Final summary: one-line-each findings fixed (+file),
behaviors verified clean per lens, perf numbers red→green, design-drift items
closed against principle IDs, honest caveats (surfaces deliberately not
reached: real global-input synthesis, unauth LLM paths, permission-gated
features), ledger closure state, and final `agent-cargo.sh test`/`clippy`/`fmt`
status. Optionally render to `.notes/chaos-findings-site/` for sharing.

**Cleanup gate:** after any UI pass — `escape → hide → getState` shows
`windowVisible: false`; `driver.close()` in try/finally; reap orphan
processes. Leaving the app visible is itself a reportable failure.

---

## Gotchas (hard-won; ignore at your own cost)

- A "hanging" build/test is usually the cargo lock or machine load — check
  `uptime` and existing locks before diagnosing a deadlock.
- Rebuilding the binary drops the macOS screen-capture (TCC) grant —
  `captureScreenshot` fails; fall back to protocol receipts.
- `simulateKey`/`setAgentChatInput` do NOT arm @mentions — only
  `simulateGpuiEvent` is real dispatch.
- `setAgentChatTestFixture` appends a turn footer that can silently push a
  "below-threshold" transcript over `is_scroll_heavy()` — keep seeded
  list-like lines low.
- `waitForSettle` replaces sleeps; `settleIsProof: false` for native input —
  demand delivery evidence.
- Sampling an idle window yields wait/kernel leaves only — sample WHILE
  driving input.
- A leftover popup/menu eats keystrokes — if input seems inert, dismiss the
  overlay and re-read state first.
- Don't trust `SCRIPT_KIT_AGENT_CHAT_RENDER_TRACE` for perf verdicts — it
  times render() body only; trust `sample`.
- The existing contract-test corpus is intent, not precedent — chaos around
  its edges, and never appease a failing lock without understanding why it
  exists.

Now go walk every story and break it on the way — sandboxed, through the
wrapper, with a receipt for every claim and a lock waiting for every fix.

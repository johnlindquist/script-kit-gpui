# AI Reliability Rules

Every AI surface in Script Kit — Quick AI, Flow chat, Agent Chat — eventually
fails: an engine dies, a token expires, a model disappears. These rules exist so
the user meets the *same* honest, actionable experience every time, and so an
agent can prove it did.

They were derived from real defects. Each rule names the defect it prevents.

---

## Rule 1 — A fact must never be re-derived from prose

**Rule.** When the runtime tells us *what* happened, classify from that fact.
Never format the fact into a sentence and hand the sentence to
`classify_provider_failure`.

`classify_provider_failure` pattern-matches English. It exists for one job:
turning an opaque provider payload into a best guess. It is not a general
classifier, and it will return `AiFailureKind::Unknown` for anything it does not
recognise.

**Use the typed entry point that matches the fact:**

| Fact you have | Use |
|---|---|
| The runtime emitted `SetupRequired` | `classify_setup_required` |
| A process failed to spawn | `classify_spawn_failure` |
| A child exited | `classify_process_failure(ChildExited { exit_code, signal })` |
| IO against a live child failed (broken pipe, closed channel) | `classify_runtime_closed` |
| Our own stable code (`quick_ai_*`) | `quick_ai_turn_failure` |
| An opaque provider payload, and nothing better | `classify_provider_failure` |

**Defects this prevents.**

- `SetupRequired { reason: "login required" }` was formatted as *"Pi Agent Chat
  setup required: login required. Available methods: browser"*. No auth wording
  the classifier recognises, so it became `Unknown` and the user saw the generic
  "The AI request did not finish" card — **with no Sign In button, on the one
  failure a Sign In button fixes.**
- A codex binary that exited on launch produced *"codex app-server exited — send
  again to reconnect"* → `Unknown` → no reconnect path.
- A pi child that died produced *"Broken pipe (os error 32)"* → `Unknown`.

**Exception, and it matters.** When the runtime's stderr says *why* it died,
that is real evidence and it must still win. `read_stdout` in
`ai/agent_chat/pi/runtime.rs` keeps the provider classifier when a stderr hint
exists ("No API key found for provider anthropic" must stay
`AuthenticationMissing`) and only falls back to `RuntimeClosed` when there is no
evidence at all.

---

## Rule 2 — Safe copy is not evidence

**Rule.** Once a failure is classified, carry the `AppFailureRecord`. Never
reduce it to its user-facing string and re-classify that string later.

The record's `primary_message()` is deliberately generic, reassuring English
("The AI connection stopped. Your work is saved and can be recovered."). Feeding
it back into the classifier always yields `Unknown`, because safe copy contains
no provider evidence by design.

**Defect this prevents.** The warm session classified a failure correctly, threw
away everything but the safe copy, and `warm_recovery_state` re-classified that
copy — so a "sign in required" warm failure arrived at its own recovery card as
`Unknown`, without the Sign In action it had a moment earlier.

**Structural enforcement.** `WarmSlot` and `AgentChatWarmSessionSnapshot` carry
`failure: Option<AppFailureRecord>`, not `failure_message: Option<String>`.
There is no longer a field to put a raw string in. Prefer this shape for any new
carrier: make the round-trip unrepresentable rather than documented.

Locked by `warm_recovery_keeps_the_typed_failure_instead_of_reclassifying_its_own_copy`,
which asserts *both* halves — the typed record produces Sign In, and
re-classifying its own copy still yields `Unknown`.

---

## Rule 3 — Raw payloads stop at the diagnostic vault

**Rule.** Raw provider text, stderr, OS errors, and adapter internals go into
the diagnostic vault. What reaches the screen is `primary_message()`. What
reaches a log line is the failure code plus the diagnostic fingerprint.

Classifying does not mean discarding: `classify_spawn_failure` and
`classify_runtime_closed` exist precisely so the cause survives in the vault
while the *kind* comes from the fact.

**Defects this prevents.**

- Once the agent-chat adapters returned the typed `AiAdapterResult`,
  `format!("Failed to prepare session: {error:#}")` printed `AiAdapterError`'s
  `Display` — the internal marker `ai_adapter_error:Unknown` — straight into the
  Agent Chat surface.
- The mini text-rewrite variation card rendered the literal internal string
  `setup_required:<reason>`.
- `prompt_handler` logged the full raw SDK error; it now logs `error_len`.

**Details dialogs** show code + summary + diagnostic fingerprint. See
`flow_recovery_copy_details` for the canonical shape.

---

## Rule 4 — Recovery actions must be performable

**Rule.** A surface declares what it can actually do via
`SurfaceRecoveryCapabilities::only([...])`. The shared reducer plans the
options; the surface filter keeps the card honest. An action the surface cannot
perform is never rendered enabled.

**Corollary that has bitten twice.** Availability is derived from wiring. If a
surface does not install its recovery callback, `turn_recovery_capabilities`
reports only `CopyDetails` and **every button that could fix the failure
silently disappears**. `with_recovery_callback` had zero callers before S10.

If a capability list omits the action that repairs a given failure, the card
renders on exactly that failure with nothing useful on it —
`RecoveryActionKind::RethreadFlow` was missing from the flow surface, which is
the action a dead flow engine needs.

---

## Rule 4a — A recovery action lives in a footer, a menu, or a modal

**Rule.** This app has exactly two sanctioned homes for a button:

1. a **modal**, for a decision the user must make before continuing;
2. a **floating footer** or the **actions menu**, for everything else.

A recovery card is a *message* — what broke, and what was preserved. It renders
no buttons. `render_ai_recovery_card` does not even take an action handler, and
`src/components/ai_recovery.rs` no longer imports `Button`, so the compiler
rejects the regression before any test runs.

Placement is decided by one pure function,
`src/ai/reliability/placement.rs::plan_recovery_presentation`:

| Role / layout | Home |
| --- | --- |
| Primary | Footer — the `↵` affordance |
| Secondary, Diagnostic | Actions menu |
| Any role on `BlockingPanel` | Modal (a blocking panel already IS a must-decide) |

**Corollary — never trade a visible action for a dead promise.** Moving an
affordance is allowed; leaving the user with a promise nothing keeps is not.
Two failure modes, both shipped here and both caught:

- advertising `⌘K Options` when no actions dialog is wired to it;
- letting Secondary/Diagnostic actions vanish because the card stopped drawing
  them and the menu never started.

`render_ai_recovery_footer` therefore branches on whether a menu is actually
reachable, and falls back to listing the actions in the rail. Prefer an
affordance that is slightly wrong in *placement* over one that is silently
*missing*. Locked by
`no_action_becomes_unreachable_when_the_surface_has_no_actions_dialog` and
`tests/source_audits/ai_recovery_button_placement.rs`.

---

## Rule 5 — A user Stop is not an error

**Rule.** Cancellation is a truthful outcome, not a failure. It gets quiet
stopped copy and never the shared error card. See `FlowReliability::cancel_turn`
and `AiTurnRuntimeOutcome::Cancelled`.

---

## Rule 6 — If a probe cannot see it, it is not proven

**Rule.** `collect_visible_elements` is a hand-written model of each surface,
not a walk of the GPUI tree. A card that renders perfectly can still be
invisible to `getElements`, which makes every "is it on screen?" assertion
**unfalsifiable** — it fails whether the card rendered or not.

Any new shared AI surface element must be projected into the element collector
from the *same* source the renderer consumes. `ai_recovery_elements` does this
for the recovery card via `recovery_semantic_tree`, so proof and render cannot
drift.

**Probe hygiene learned the hard way:**

- Always write the receipt, including on a thrown step. A probe that throws
  before its write leaves the *previous* run's receipt on disk, and it reads as
  fresh evidence.
- Poll for state; never sleep a fixed interval and assume. A fixed sleep after
  Enter meant a slow launch fired Enter at whatever row happened to be selected.
- Record what *was* there, not only what was missing. A failure listing the full
  element id set is diagnosable; "not found" is not.
- Kill every engine the surface might resolve to. A probe that replaced only the
  pi binary silently exercised the codex path instead.

---

## Focused checks

```bash
# The classification rules (Rules 1-3)
./scripts/agentic/agent-cargo.sh test --lib ai::reliability

# The reducer's recovery planning (Rules 4-5)
./scripts/agentic/agent-cargo.sh test -p sk-protocol

# The surfaces
./scripts/agentic/agent-cargo.sh test --lib "ai::agent_chat"
./scripts/agentic/agent-cargo.sh test --lib "flows::"
./scripts/agentic/agent-cargo.sh test --lib "prompts::chat"
```

Runtime proof (Rule 6) needs a real build:

```bash
SCRIPT_KIT_AGENT_ARTIFACT_NAME=ai-rock-solid \
  ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui
export SCRIPT_KIT_GPUI_BINARY="$PWD/target-agent/artifacts/ai-rock-solid/script-kit-gpui"

bun scripts/agentic/flow-ai-recovery-probe.ts        # flow surface, dead codex
bun scripts/agentic/ai-recovery-surface-film.ts      # flow + Agent Chat, compared
bun scripts/agentic/agent-chat-auth-recovery-probe.ts
bun scripts/agentic/agent-chat-retry-recovery-probe.ts
bun scripts/agentic/quick-ai-policy-probe.ts
```

Receipts land in `.test-output/ai-rock-solid-ux/`, screenshots in
`.test-screenshots/ai-rock-solid-ux/`.

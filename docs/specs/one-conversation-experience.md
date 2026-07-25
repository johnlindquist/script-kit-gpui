# One conversation experience — Quick AI, Flow chat, Agent Chat

**Status:** measured audit + build order. Nothing here is built yet.
**Scope:** the everyday, working path. Failure and recovery are already unified —
see `rules/AI_RELIABILITY.md` and the receipt in §0.

## The user's ask

> Quick AI, Flow chat, and Agent Chat should have the same conversation
> experience, while keeping their different purposes: Quick AI runs the fastest
> models, Flow chat carries flow logic, Agent Chat runs profiles that build
> things.

Parity level chosen: **100 — "one conversation experience."** The distinctions
live in what each surface can DO, not in how it behaves.

---

## 0. What is already one experience

The failure path. Filmed on 2026-07-24 with dead engine binaries on both
surfaces (`scripts/agentic/ai-recovery-surface-film.ts`, receipt
`.test-output/ai-rock-solid-ux/ai-recovery-surface-film.json`, status `green`):

| Surface | Failure code | Recovery card nodes |
|---|---|---|
| Flow session | `ChildExited` | card, title, body, retry, **rethread-flow**, repair-component, copy-details, dismiss |
| Agent Chat | `RuntimeClosed` | card, title, body, retry, repair-component, copy-details, dismiss |

Same card, same safe copy, same semantic ids. The one difference — Rethread
flow — is a real capability difference, which is exactly the shape the rest of
this document is aiming for.

**That is the target pattern:** shared anatomy, capability-shaped actions.

---

## 1. The structural fact underneath everything

There are **two independent conversation implementations**, and neither knows
about the other.

| | Flow chat (and mini AI) | Agent Chat (and Quick AI) |
|---|---|---|
| Transcript | `src/prompts/chat/**` — `ChatPrompt`, ~5,600 lines | `src/ai/agent_chat/ui/components/transcript.rs` — 2,568 lines |
| View | rendered inside the flow built-in, `src/render_builtins/flow_ux.rs` | `src/ai/agent_chat/ui/view.rs` — 16,898 lines |
| Composer | the **MAIN window input** (`ChatPromptHostMode::TranscriptOnly`, `flow_ux.rs:876`) | its own composer, `ui/components/composer.rs` + view.rs |
| Style source | local `CHAT_LAYOUT_*` consts in `src/prompts/chat/mod.rs` plus inline `px(...)` literals in `render_turns.rs` | a declarative `AgentChatStyleDef` / `production_agent_chat_style()`, `ui/style_contract.rs:117` |
| Markdown | a local `render_markdown(source, colors)` helper | `TextView::markdown` with a cached `TextViewStyle` (`transcript.rs:756`) |

The numbers currently agree by coincidence, not by construction:
`CHAT_LAYOUT_CARD_PADDING_X/Y = 12.0 / 10.0` and
`AGENT_CHAT_INPUT_PADDING_X/Y = 12.0 / 10.0` are the same values written twice,
in two files, with no test tying them together. Either can drift silently.

**Consequence for planning:** "one conversation experience" is not a styling
pass. It is deciding which of the two implementations survives, and porting the
other surface onto it. Everything in §3 is ordered around making that possible
without a big-bang rewrite.

---
## 1a. Correction — three surfaces are really two, plus a fourth

Quick AI and Agent Chat are **the same renderer**. They differ only by
`AgentChatCapabilities` (`ui/capabilities.rs:249`) and
`ChromeDensity::Compact` (`ui_variant.rs:184`). Every Quick-AI-vs-Agent-Chat
difference found traces to an explicit capability denial with a test behind it
— deliberate, not drift.

So roughly **90% of everyday divergence is Flow vs Agent Chat.** That is where
the work is.

There is also a **fourth surface** this document must not forget: standalone
`ChatPrompt` (`AppView::ChatPrompt`, the SDK `chat()`), which renders its own
header, composer, and footer (`render_core.rs:89`) that Flow suppresses. Any
fix that keeps `ChatPrompt` as the transcript engine drags it along. Its footer
still says `↵ Run` (`prompt_layout_shell.rs:815`) — the wrong verb for a chat.

**Already healthy:** the header/context zone agrees between Flow and the Agent
surfaces (`view.rs:16092`, `flow_ux.rs:2344`), and no surface bypasses shared
`footer_chrome`/`hint_strip`.

---

## 2. Measured divergences, ranked by what a user actually hits

### 1. Flow silently destroys a pasted multi-line message — DATA LOSS

The worst finding, and it is not a consistency issue — it is corruption.

Flow composes in the shared main-window input, which is single-line at **two**
layers:

- **vendor:** `InputState::new` defaults to `InputMode::SingleLine`
  (`vendor/gpui-component/crates/ui/src/input/state.rs:420`), and the main
  input is built bare at `src/app_impl/startup.rs:398` — no `.multi_line(true)`
  anywhere in `src/`;
- **app:** `AppView::FlowSessionView` is in
  `current_view_uses_shared_filter_input` (`filter_input_core.rs:56`), and
  `filter_input_change.rs:168` force-reverts any change containing a newline.

On paste, the vendor does this (`state.rs:1776`):

```rust
if !self.mode.is_multi_line() {
    new_text = new_text.replace('\n', "");
}
```

Newlines are **deleted, not replaced with a space**. Pasting
`"Fix the bug\nin auth.rs"` into a flow conversation sends
`"Fix the bugin auth.rs"`.

**Why it is silent:** the app layer has a guard that logs
`filter_change.newline_ignored` — so the case looks handled. But the vendor
strips the newlines *before* any change event fires, so that guard never runs
and nothing is logged. The user sees text arrive and has no reason to look.

Shift+Enter is swallowed for the same reason; Agent Chat honors it
(`view.rs:15376`).

**Converge on** Agent Chat's composer behavior.
**Size:** large — both layers must change together. Removing only the app guard
still loses newlines to the vendor strip.

#### Runtime receipt, then the fix

`scripts/agentic/flow-composer-multiline-probe.ts` pasted the message into a
real flow session and read the composer back. It confirmed the source reading
exactly:

```json
{ "pasted": "Fix the bug\nin auth.rs",
  "composerAfterPaste": "Fix the bugin auth.rs",
  "verdict": "corrupted-newline-deleted" }
```

The corruption is now fixed at the vendor layer. `paste()` calls
`flatten_line_breaks_for_single_line`, which collapses each **run** of line
breaks to a single space and contributes nothing at the edges, so a line copied
from a terminal gains no trailing padding. Every word and every word boundary
survives; no word is invented.

That is the right behavior for *every* single-line input in the app, not just
Flow — deleting a newline is never what a user meant, and welding two words is
strictly worse than flattening. Locked by four unit tests in
`single_line_paste_tests` (`state.rs`).

**Status: data loss closed.** Line *structure* is still lost, because Flow
still composes in the shared single-line input. Shift+Enter likewise inserts
nothing (`shiftEnterInsertedNewline: false` in the receipt). Both need a
dedicated Flow composer and belong to §2 item "unify the AI composer" — a much
larger change, since the shared input is also ScriptList's, where Enter must
keep meaning *select*. The probe reports that residual gap as
`structureLossStillOpen: true` rather than letting a flattened run read as
complete.

### 2. Escape means opposite things while streaming

**Corrected 2026-07-25.** The first pass of this document said the two keys
were *swapped* between surfaces. They are not. `⌘.` already means Stop on both.

In `handle_key_down` (`view.rs:14658`), the `⌘.` cancel-streaming branch sits at
`:14975` and the reopen-focused-mention branch at `:15231` — same function, no
boundary between them, so **stop wins whenever a turn is streaming**. Reopen is
additionally gated on `open_focused_mention_portal(cx)` returning true, so it
only fires when not streaming *and* a mention is focused.

What actually differs is Escape alone:

| | Agent Chat | Flow |
|---|---|---|
| `Esc` mid-stream | **stops the model** (`view.rs:15457`) | **abandons the surface; the model keeps running** (`flow_ux.rs:167`) |
| `⌘.` mid-stream | stops (`view.rs:14975`) | stops (`flow_ux.rs:170`) — **already agree** |

Flow's stop was also undiscoverable until this pass: it appeared nowhere but
inside `⌘K` (`actions.rs:1053`).

**Converge on** the Agent Chat ladder — Esc #1 stops, Esc #2 leaves — keeping
background as rung two. **Size:** small, and smaller than first thought: only
Escape moves, `⌘.` is already consistent.
**Addressed:** the flow footer now advertises `⌘. Stop` while working, and
`agent_chat_cmd_period_stops_streaming_before_reopening_a_mention` locks the
precedence that makes `⌘.` trustworthy on both surfaces.

**Footer grammar converged.** Agent Chat's footer used to advertise `Esc Stop`
while Flow's said `⌘. Stop`. Each was honest about its own surface, which is
what made the split dangerous: a user who learned "Esc stops the model" in
Agent Chat and applied it in Flow *backgrounded the session and left the turn
running*. Both footers now name `⌘.`, the one chord that already stops on
both, via `FOOTER_AI_STOP_KEY`/`FOOTER_AI_STOP_LABEL` in
`components/footer_chrome.rs`.

Escape's *behavior* is deliberately untouched here — it still stops in Agent
Chat and backgrounds in Flow. That divergence is the Escape modal's to
resolve (`docs/specs/backgrounded-ai-sessions.md`), and naming `⌘.` in the
footer is forward-compatible with it: once Escape means "Background or Close"
everywhere, `⌘.` is already established as the unambiguous Stop.

Locked by `agent_chat_and_flow_advertise_the_same_stop_chord`, which renders
Agent Chat's Stop hint through Agent Chat's own label mapper and compares it
to the string Flow actually shows, so re-inlining a literal on either side
fails rather than shipping as a quiet split.

### 3. Flow's ⌘K is missing nearly every everyday action

Agent Chat offers ~20 (`script_context.rs:974`); Flow offers **six**
(`actions.rs:1018`): Open, Background, Copy Transcript, New Flow, Stop Turn,
Terminate.

Flow has none of: Copy Last Response, Copy as Markdown, Copy All Code Blocks,
Save as Note, Retry, Scroll to Latest.
**Converge on** the Agent Chat vocabulary, keeping Background/Terminate/Rethread
as a Flow-only section. **Size:** medium.

**First verb landed: Copy Last Response.** It is the most common thing a user
does with a finished turn, and Flow's only copy was "Copy Transcript" — paste
everything, then hand-delete back to the one answer you wanted. Flow now
mirrors Agent Chat exactly: same title, same `⇧⌘C`, same `Response` section
(which also adopts Copy Transcript, so both copy verbs are found together).

The handler deliberately walks turns in reverse for the newest **non-empty**
assistant text rather than taking `turns.last()`. The in-flight turn carries an
empty `assistant` until the engine replies, so the naive version would copy an
empty string mid-stream and report success — the same "looks handled, is not"
shape as the paste bug above. With no answer yet it says so instead.

Locked by `flow_sessions_copy_the_last_response_the_same_way_agent_chat_does`.
Remaining from this item: Copy as Markdown, Copy All Code Blocks, Save as Note,
Retry, Scroll to Latest.

### 4. Assistant text is selectable in Agent Chat, not in Flow

Agent Chat renders markdown through gpui-component `TextView` with
`.selectable(true)` (`transcript.rs:898`). Flow uses a separate engine
(`src/prompts/markdown/api.rs:7`) with **zero** `selectable` and zero `TextView`
across all 11 of its files. You cannot select an answer to quote it.
**Size:** large — also hits standalone `ChatPrompt`.

### 5. Copy affordances are inverted

Flow has a **per-turn copy button** (`render_turns.rs:158`) that Agent Chat
lacks entirely — Agent Chat's only copy is `⌘⇧C` for the last message, and
`transcript.rs` has exactly three `on_click` sites, none of them copy.
Conversely Agent Chat has first-class code-block copy (`transcript.rs:845`)
while Flow's is a differently-styled hover reveal (`code_table.rs:81`).

**Neither surface is the winner. Adopt both, everywhere.** Size: medium.

### 6. Jump-to-latest exists only on the weaker surface

Flow has the pill (`render_core.rs:469`) plus End/`⌘↓`. Agent Chat has none —
"Jump to latest" appears at exactly one place repo-wide. Port it to Agent Chat
against `ListState::is_following_tail()` (`transcript.rs:719`) so it stays
truthful. **Size:** small.

### 7. Up-arrow prompt history is dead in Flow

Agent Chat recalls previous prompts (`view.rs:15112`). The arrow interceptor has
an arm for `FlowUxView` but none for `FlowSessionView`, so it falls to the
catch-all. **Size:** small.

**Landed.** Up/Down now walks a flow session's prompt history, on shell rules:
Up from the draft recalls the newest prompt, Up clamps at the oldest rather
than wrapping, and Down past the newest restores the draft the user was
typing — so recall is always reversible without retyping.

Two decisions worth keeping:

- **The history is the session's own turns.** Recall reads `turn.user`; the
  app stores only a cursor into it. A second copy would disagree with the
  transcript after a rethread or a failed turn.
- **The handler sits BEFORE the big `match &mut this.current_view`**, not in an
  arm of it. That match holds a mutable borrow of the whole app for its
  duration, so an arm cannot call `set_filter_text_immediate`; deferring the
  composer write instead would let it land after the next keystroke.

Semantics are locked by four tests in `flow_ux.rs`, including one that sweeps
every (history length, cursor, direction) combination to prove no recalled
index can ever be out of range — an off-by-one there would panic on subscript
or silently recall the wrong prompt.

### 8. Sending while busy: queued vs error toast

Agent Chat queues the message and shows a queue strip (`view.rs:11664`). Flow
rejects it with an **error toast** (`flow_ux.rs:978`). An error is the wrong
register for "you typed ahead", and the footer already hides `↵ Send` while
busy, so it is a double negative. **Size:** medium.

### 9. Your own message looks completely different

Agent Chat: a tinted bubble, per-message row (`transcript.rs:1209`). Flow: a
small bold grey line inside a fused per-turn card (`render_turns.rs:45`).

**Converge on the per-message row model** — it is the one that scales to the
Thought / Tool / Error / attachment rows Flow cannot express at all.
**Size:** large — same work as #4, do them together.

### 10. Flow has no context attachment and no way to start a fresh conversation

`⌘N`/`⌘L` are unbound in Flow; a fresh thread is reachable **only through a
failure recovery card** (`RethreadFlow`, `flow_ux.rs:2436`). And the comment at
`flow_ux.rs:2234` claiming the shared main input brings "all its
context-attachment features" is **aspirational, not true** — the attachment
portal is ScriptList-scoped (`attachment_portal.rs:6`).
**Size:** small for New Conversation (reuse the `RethreadFlow` machinery);
large for attachments.

### Also true, lower impact

- **Composer:** three different text-input implementations (Agent Chat's
  app-local `TextInputState`, Flow's shared gpui-component `Input`, standalone
  `ChatPrompt`'s own). Placeholders diverge: `"Ask anything…"`/`"Follow up…"`
  vs `"Message {flow}…"`. Flow has no send button at all
  (`flow_ux.rs:2241`), and its draft *is* the global filter text, cleared on
  open and on background.
- **Streaming indicator:** a pulsing dot and "Thinking…" row
  (`transcript.rs:1055`) vs a markdown literal `_Thinking…_`
  (`render_turns.rs:7`). No surface shows elapsed time or token count.
- **Style:** `AgentChatStyleDef` is a real style contract
  (`style_contract.rs:117`); ChatPrompt has none, only `CHAT_LAYOUT_*` consts
  and inline `px()` literals. `CHAT_LAYOUT_CARD_PADDING_X/Y` and
  `AGENT_CHAT_INPUT_PADDING_X/Y` are both `12.0/10.0` — the same values written
  twice, tied by nothing. `src/design_contract/mod.rs` registers both
  vocabularies (`:3897`, `:3526`), documenting the split rather than closing it.

### Evidence standard

Every negative claim above was proven by enumeration, not by a failed grep:
the three `on_click` sites in `transcript.rs`, the single repo-wide
"Jump to latest", zero `selectable`/`TextView` across all 11 markdown files,
all 14 `AppView` arms in the arrow interceptor, and the single-child Flow body
slot. All evidence is static source. Two behaviors still deserve a runtime
probe: Shift+Enter reaching the vendored input, and the finding-#1 paste
corruption.

## 3. Build order

Revised after the full sweep. The rule that keeps this landable:
**stop the bleeding, then behavior, then pixels.**

0. **Stop destroying pasted messages** (§2.1). This is data loss, not
   inconsistency, and it outranks everything else here. Both layers together:
   make the main input multi-line-capable for flow sessions, and drop the
   app-side newline revert for `FlowSessionView`. *Proof:* a devtools probe
   that pastes multi-line text into a flow session and reads the composer back
   — the finding is source-verified but has no runtime receipt yet.
1. **One meaning per key** (§2.2): Esc #1 stops, Esc #2 leaves; `⌘.` stops
   everywhere. *Proof:* per-surface key-ladder tests plus a streaming probe, on
   all three keyboard paths (`flows/escape.md` — automation only exercises one,
   so a partial fix looks green).
   - **Landed (half):** the flow footer now advertises `⌘. Stop` while working,
     in both renderings, and the native button reaches `stop_flow_session`. The
     binding always existed; the footer never named it.
2. **The small, high-value ports** — each independently shippable:
   jump-to-latest into Agent Chat (§2.6), Up-arrow history into Flow (§2.7),
   per-turn copy into Agent Chat and code-block copy into Flow (§2.5),
   New Conversation into Flow (§2.10, reuse the `RethreadFlow` machinery).
3. **Queue instead of scold** (§2.8): a busy Flow send joins a queue rather
   than raising an error toast.
4. **One footer grammar function**, taking capabilities + turn state. Retire
   `Esc Desk`; fix standalone `ChatPrompt`'s `↵ Run`.
   - **Started:** `flow_session_native_footer_and_hint_strip_agree_on_the_same_grammar`
     ties the two flow renderings together; it does not yet unify Flow with
     Agent Chat.
5. **`ConversationStyle`** — widen `AgentChatStyleDef`, delete the
   `CHAT_LAYOUT_*` duplicates, collapse the two design-contract vocabularies
   into one.
6. **Flow's ⌘K vocabulary** (§2.3) — cheaper once the shared actions exist.
7. **The renderer port** (§2.4 + §2.9 together): move Flow onto the
   `TextView` markdown pipeline and the per-message row model. Largest, last,
   and it drags standalone `ChatPrompt` along.

## 3a. A formatting trap worth knowing about

`cargo fmt` in this repo wants a different style edition than the checked-in
code: running it on `src/render_builtins/flow_ux.rs` or
`src/ai/agent_chat/pi/runtime.rs` reorders import blocks and rewraps untouched
method chains, producing large diffs that have nothing to do with the change.
The pre-commit hook's own format check passes on the *existing* style.

So: format, then revert the hunks you did not intend. `git diff > patch`,
filter to your hunks, `git checkout --` the file, `git apply` the filtered
patch. Check the result with `git diff | grep '^@@'` before committing — the
hunk list should be short enough to read.

## 4. Constraints carried in

- **Glass Motion Calibration Lock** (`CLAUDE.md`): none of this authorizes
  retuning entry/exit motion. Reuse the calibrated surfaces.
- **Shared component contract** (`CLAUDE.md`): extend `src/components/**` and
  the theme/token layers rather than adding surface-local helpers. Any
  intentional divergence must be documented with the alternatives considered.
- **Source Audit Test Policy**: prefer the compiler and behavior tests. A
  "no bare `px(` in conversation renderers" check is a source audit and is only
  justified once the shared token exists for it to point at.
- **The Escape design is coupled**: `docs/specs/backgrounded-ai-sessions.md`
  puts a Background/Close modal on the last Escape rung. Step 1 here decides
  what the earlier rungs mean. Land them in that order.

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

## 2. Measured divergences, ranked by what a user actually hits

Each row was read out of the source; the anchors are exact. Rows marked
**unverified** are named honestly rather than guessed at.

### 1. The same keys mean different things — the worst one

| Key | Flow chat | Agent Chat |
|---|---|---|
| `Esc` while streaming | **Background the session** (`FlowSessionKeyAction::Background`, `flow_ux.rs:167`) | **Stop the turn** — the footer literally says `Esc Stop` (`view.rs:4104`) |
| `⌘.` | **Stop the turn** (`FlowSessionKeyAction::Stop`) | **Reopen the focused mention** (`is_reopen_focused_mention_shortcut`, `view.rs:12544`) |

So the two keys are *swapped* between surfaces, and `⌘.` in Agent Chat does
something unrelated to stopping. A user who learns "⌘. stops it" in a flow
conversation will, in Agent Chat, open a mention popup while the model keeps
streaming.

**Converge on:** `⌘.` = Stop everywhere. Escape stays the leave/unwind ladder.
This must be decided together with `docs/specs/backgrounded-ai-sessions.md`,
whose Escape modal sits on the LAST rung of that ladder — it must not swallow
"Esc Stop".
**Size:** medium. Three parallel keyboard paths must change in lockstep
(`flows/escape.md`: capture interceptors in `startup.rs`, per-surface bubble
handlers, and the automation mirror in `simulate_key_dispatch.rs`) — automation
probes only exercise the third, so a partial fix looks green.

### 2. The footer describes a different app on each surface

- Flow: `↵ Send · ⌘K Actions · Esc Desk` idle; `⌘K Actions · Esc Desk` while
  working — **no Stop hint at all**, on the surface where `⌘.` is the stop key
  (`flow_session_footer_hints`, `flow_ux.rs:118`).
- Agent Chat: a `FooterAction` label table — `↵ Send`, `Esc Stop`, `⌘K Actions`,
  `⌘W Close`, `📁 CWD`, `⇧⇥ Agent`, … (`footer_hint_label`, `view.rs:4097`).

`Esc Desk` is also the "lying footer" already flagged in the execution ledger:
Escape does not go to a desk, it backgrounds the session.

**Converge on:** one footer grammar function that both surfaces call, taking
capabilities and turn state, so a hint cannot exist without the binding that
performs it.
**Size:** small-to-medium.

### 3. Two markdown pipelines render the same assistant text

Agent Chat uses `TextView::markdown` with a cached `TextViewStyle` and a
`MarkdownLinkLabelPolicy`; ChatPrompt calls a local `render_markdown` helper.
Code blocks, link handling, paragraph spacing, and heading scale are therefore
independently defined.

**Converge on:** Agent Chat's. It is the one with a declared style contract,
link policy, and a style cache tuned for streaming re-renders.
**Size:** large — this is the core of the port.

### 4. Two style vocabularies for the same surface

`AgentChatStyleDef` is a real, testable style contract
(`AgentChatTranscriptStyle`, `AgentChatMarkdownStyle`, `AgentChatMessageStyle`,
`AgentChatCollapsibleStyle`, `AgentChatErrorStyle`, `AgentChatSystemStyle`).
ChatPrompt has none: `render_turns.rs` hardcodes `px(6.0)`, `px(8.0)`,
`px(24.0)`, `px(64.0)`, `rounded(px(8.0))` inline.

Both sets of constants are registered in `src/design_contract/mod.rs`, as two
separate vocabularies (`prompts::chat::CHAT_LAYOUT_*` at `:3897` and
`style_contract::AGENT_CHAT_*` at `:3526`) — so the design contract currently
*documents* the split rather than closing it.

**Converge on:** extend `AgentChatStyleDef` into a surface-neutral
`ConversationStyle` and have both renderers read it. Per the repo's UI contract,
tokens live in the shared layer, not in surface renderers.
**Size:** medium.

### 5. The composer is a different component

Flow's composer IS the main window input (`TranscriptOnly` host mode). Agent
Chat has its own, at `AGENT_CHAT_INPUT_FONT_SIZE = 17.0` /
`LINE_HEIGHT = 22.0` (`style_contract.rs:206`, `:210`).

Placeholders diverge in wording and in kind: Flow sets
`"Message <friendly>…"` (`flow_ux.rs:1570`), Agent Chat uses
`"Ask anything…"` / `"Follow up…"` (`style_contract.rs:217`, `:220`).

**Unverified:** whether the two render at the same visual size. The main input
resolves its size through the chrome/typography layer, not through a literal, so
comparing `17.0` against it needs a measurement, not a grep. Do that before
changing any number.
**Converge on:** one placeholder rule — name the counterpart on first message,
"Follow up…" after — and one measured type scale.
**Size:** small for the copy, medium for the geometry.

### 6. Dimensions not yet measured

Named so they are not mistaken for "no divergence found": scroll/autoscroll
behavior and scrollbar, `⌘K` action sets per surface, message history
(Up/Down), copy-last-message, new-conversation, and `@`-mention/attachment
support per surface. Each needs the same treatment as rows 1–5 before it can be
ranked.

---

## 3. Build order

The rule that keeps this landable: **behavior before pixels.** Keyboard and
footer divergences are what a user actually trips over, and they are small.
The renderer port is large and should not block them.

1. **`⌘.` = Stop on every AI surface**, and give the flow footer its Stop hint
   while working. *Proof:* per-surface key-ladder tests plus a probe that
   streams a turn and presses `⌘.`, on all three keyboard paths.
   - **Landed (second half only):** the flow footer now shows `⌘. Stop` while
     working, in both renderings, and the native button reaches
     `stop_flow_session`. The binding already existed — the footer simply never
     named it. Agent Chat's `⌘.` still means "reopen focused mention"; making
     the key itself agree is the remaining, larger half.
2. **One footer grammar function**, taking capabilities + turn state. Retire
   `Esc Desk`. *Proof:* a table test enumerating (surface × state) → hints, and
   an assertion that every hint's key resolves to an installed binding.
   - **Started:** `flow_session_native_footer_and_hint_strip_agree_on_the_same_grammar`
     ties the two flow renderings together. It does not yet unify Flow with
     Agent Chat — that needs the shared builder.
3. **`ConversationStyle`** — widen `AgentChatStyleDef`, delete the
   `CHAT_LAYOUT_*` duplicates, collapse the two design-contract vocabularies
   into one. *Proof:* the existing design-contract tests, now over one
   vocabulary; a test that no conversation renderer contains a bare `px(` in a
   spacing position.
4. **Measure the composer**, then unify placeholder copy and type scale.
   *Proof:* a screenshot pair at the same viewport plus the measured values.
5. **Port ChatPrompt's transcript onto the Agent Chat markdown pipeline.** The
   big one; do it last, behind the shared style from step 3.
6. **Measure §2.6**, then re-rank.

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

# Handoff — AI consistency + the button placement rule (2026-07-25 PM)

Written by the lane that answered your `/align` page. Read "What changed" and
"What I could not prove"; the rest is reference.

Authorization: `.hitl-align/ai-consistency-buttons/submissions/ceddf574-f524-4f5a-8a8b-984b87ca860c.json`
(buttons = footer-and-actions, blast = all-surfaces, next-split = dedupe,
scope = 100, autonomy = 100).

---

## The finding behind it

Your side note was right, and **wider than Quick AI**.

Quick AI is not its own renderer. It is an Agent Chat thread with a `QuickAi`
policy (`src/ai/agent_chat/ui/thread.rs:589`), and its errors go through one
shared component — `src/components/ai_recovery.rs`, which painted real
`Button` elements *inside the message body* (L180 per action, L233 for
Dismiss). Three mount sites, all in-window.

The layout picker offered four placements — `ComposerInline`, `TranscriptCard`,
`DeskRow`, `BlockingPanel`. **All four were inline. Neither of the two homes
you named existed as an option.**

So it was one component to fix, not six surfaces.

---

## What changed

| Commit | What |
| --- | --- |
| `b6d0459f8` | The pure placement rule. Closed enum with **no `Inline` variant**; total `(layout, role) -> home`; a plan value a surface mounts without a window. |
| `4acdae290` | The card becomes message-only. `handlers` parameter deleted, `Button` import dropped — **the compiler now rejects a button in that file**. All three surfaces wired in one commit. Guard audit added. |
| `2a918ed8f` | **Self-correction.** `4acdae290` advertised `⌘K Options` bound to nothing and made Copy details unreachable. Footer now branches on whether a menu is really wired. |
| `e7f7c4150` | Flow uses the **shared** per-turn copy button instead of its own identical copy. |
| `ab5389e85` | Flow's jump pill reads `ListState::is_following_tail()` — one follow-tail authority, same as Agent Chat. |

Docs: `rules/AI_RELIABILITY.md` gains **Rule 4a**, which states the two
sanctioned homes and the never-trade-a-visible-action-for-a-dead-promise
corollary.

### The commands that verify it

```bash
./scripts/agentic/agent-cargo.sh test --lib ai::reliability::placement            # 9 passed
./scripts/agentic/agent-cargo.sh test --test source_audits ai_recovery_button_placement  # 3 passed
./scripts/agentic/agent-cargo.sh test --bin script-kit-gpui flow_                # 90 passed
./scripts/agentic/agent-cargo.sh test --lib chat                                 # 1420 passed
./scripts/agentic/agent-cargo.sh test --lib ai::reliability                      # 27 passed
```

`tests/source_audits` full run: **721 passed / 35 failed** — the same 35 that
were failing before this work (baseline was 718/35; the three new ones pass).
None of the 35 is in a file this pass touched.

---

## What I could not prove

1. **Nothing was rendered.** Every claim here is compile-time or test-time. I
   never saw the new footer rail on screen, and I cannot tell you it looks
   right. This is the single biggest gap, and it is why I recommended against
   full autonomy on the page.
2. **No runtime probe ran.** The Agent Chat probe family is still dead — every
   probe calling `openAgentChatKitchenSinkFixture` fails at its first request
   because that fixture was deleted in `401936c41`. So the surface most
   affected by this change has zero runtime coverage.
3. **The footer rail is mounted under the card, not in each surface's real
   bottom footer.** It uses the sanctioned shared rail component
   (`render_clickable_footer_hint_action_rail`, the same one the main menu
   uses) with proper keycap language — but hoisting it into the actual footer
   slot of three different surfaces is a further change.
4. **`on_open_menu` is `None` at all three sites.** No surface's actions dialog
   carries recovery actions yet. The fallback keeps every action reachable in
   the rail, so nothing is lost — but the intended end state (secondary and
   diagnostic in ⌘K) is not built.
5. **`user_has_scrolled_up` still exists.** Only the *pill decision* moved to
   the shared authority. The flag still drives `set_follow_tail` and the state
   snapshot. Deleting it is its own pass.

---

## The mistake worth reading

`4acdae290` shipped both halves of the bug this repo has already shipped twice:
a chord advertised with nothing bound to it, **and** actions that silently
vanished. The error card still rendered a correct-looking message, so nothing
turned red. I caught it while writing the commit message, not while writing the
code.

The negative control for the guard audit *also* failed dishonestly the first
time: injecting `Button::new` made the crate fail to **compile**, not fail the
assertion. Exit code 101 either way. Had I not checked *why*, I would have
recorded a false green on the test whose entire job is to prevent that.

Both belong to the same family as yesterday's three:

> A value is degraded, the degraded value still looks plausible, and the code
> that would have warned never runs.

---

## Still waiting on you

| Item | What is needed |
| --- | --- |
| **Escape split** | Answers to gaps G1–G5 in `docs/specs/backgrounded-ai-sessions.md`. Escape still stops in Agent Chat and backgrounds in Flow. |
| **Composer split** | Your `"if it makes sense"` — Flow first, or all AI chats at once. |
| **Agent Chat fixture** | Re-add production-wired (undoing `401936c41`) or build a test-only fixture. Until then, no Agent Chat runtime coverage. |

## Oracle

Consults spent: **0**. I first recorded `NEEDS_DEPLOY` / "ChatGPT UI drift"
from `oracle_doctor.sh`; that was disputed and is **probably wrong** — my own
doctor output showed two healthy slots and two in cooldown, consistent with
dead pool slots rather than a broken client. I never ran `deploy-proof-now.sh`.
Corrected in full at `.notes/oracle/ai-consistency-buttons/premise.md`.

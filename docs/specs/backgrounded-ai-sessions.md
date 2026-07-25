# Backgrounded AI Sessions — design

**Status:** design only. Nothing here is built yet.
**Owner surfaces:** Quick AI, Flow chat, Agent Chat.

## The idea, in the user's words

> Escape should offer **Background** or **Close** across all the AI experiences.
> Clicking away should auto-background, and a backgrounded session becomes the
> **first option in the main menu**.

## Why this is worth designing before building

Every piece of this already half-exists, in three different shapes, with three
different owners. Building it by pattern-matching would produce a fourth shape.

- Flow sessions **already** background on Escape and **already** appear in a
  main-menu section (`prepend_root_flow_sessions_section`, "Active Flows").
- Agent Chat **deliberately survives** click-away today and must not be
  destroyed by it — two code paths depend on that.
- Quick AI **deliberately refuses** to resurrect a transcript, ever. That is a
  documented product decision, not an oversight.

So this is not "add a feature". It is "generalise one surface's behaviour to
three, and renegotiate exactly one product decision."

---

## 1. The model

One concept, `BackgroundedSession`, held on app state **outside `current_view`**
— because losing focus already destroys `current_view` for every non-ScriptList
surface (`render_impl.rs` blur handler → `close_and_reset_window` →
`reset_to_script_list`). Flow sessions survive today only because
`flow_sessions: Vec<(FlowSessionMeta, Entity<ChatPrompt>)>` lives on the app,
not the view. The new registry must live in the same place, for the same reason.

```
BackgroundedSession {
    id: SessionId,              // stable, monotonic, per-session
    surface: AiSurface,         // QuickAi | Flow { flow_id } | AgentChat { profile_id }
    title: String,              // what the user will recognise in the list
    subtitle: String,           // "<state> · <engine> · backgrounded 2m ago"
    last_activity: SystemTime,  // ordering key — see gap G1
    state: SessionLiveness,     // Live { turn_in_flight } | Idle | Failed(code)
    entity: SessionEntity,      // the retained view/thread entity
}
```

**Ordering is by `last_activity` descending, not creation order.** The existing
Active Flows section sorts by `b.id.cmp(&a.id)` — creation order — which is the
wrong key the moment a user has two sessions and returns to the older one.

**Prior art to adopt, not reinvent:** `src/ai/agent_task_dock.rs` is a complete,
tested, pure state model (`AgentTaskDockState`, `resume_task`, `archive_task`,
`AgentTaskDockSurface::{Embedded, Detached}`) with **zero call sites**. It was
built for exactly this and never wired. Start there rather than authoring a new
registry.

---

## 2. Escape → a three-way choice

### Where the modal sits

Escape is already fully bound on all three surfaces, and the ladders differ:

- **Flow session:** plain Escape *already* means Background
  (`resolve_flow_session_key_action`). `⇧⌘⎋` means Terminate.
- **Agent Chat:** a six-step progressive ladder — dismiss popup, attach menu,
  composer picker, focused-text-mini unwind, cancel streaming, *then* close.
- **Quick AI:** same view as Agent Chat, so the same ladder.
- **Mini AI (`ChatPrompt` standalone):** stop streaming, else escape.

**The modal is the LAST rung only.** It replaces the final "close" step, never
an earlier unwind step. Pressing Escape with a popup open still closes the
popup; pressing Escape mid-stream still cancels the stream. The user only sees
the choice when the alternative would have been losing the session.

### The dialog

Reuse `open_parent_action_dialog` (`src/confirm/parent_dialog.rs`). It is the
only existing modal that can express three choices — the in-window
`AppView::ConfirmPrompt` is strictly boolean (`Sender<bool>`) and cannot.

| Slot | Label | Result |
|---|---|---|
| primary | **Background** | register + hide, session stays resumable |
| secondary | **Close** | terminate the session, discard the entity |
| dismiss | **Cancel** | stay exactly where you were |

Reference call shape: `agent_chat_launch.rs`'s Retry / Details / Back dialog.

### The skip rule

A modal on every Escape would be intolerable. **The dialog appears only when
backgrounding would actually preserve something**: a turn in flight, a non-empty
transcript, or a non-empty draft. An empty, untouched session closes silently on
Escape, exactly as today.

### `⇧⌘⎋` keeps meaning Terminate

It already does, on flow sessions. It becomes the documented "Close without
asking" escape hatch for all three surfaces.

---

## 3. Click-away → auto-background

**No dialog on blur.** The user did not ask a question by clicking away; asking
one back is rude.

**"Clicking away" is three different events, not one** (ruled 2026-07-25,
submission `98cab5e5-…641` + Oracle `floating-capsule-entry-material`):

1. **Modal backdrop click** — dismisses that modal ONLY. The topmost modal
   owns the click, consumes it, and restores parent focus. It never
   backgrounds the underlying session. This is already the shipped behavior
   (`src/render_prompts/arg/helpers.rs` closes the actions popup on backdrop
   click) and the user confirmed it as the general rule.
2. **In-app click outside a non-modal surface** — follows that surface's
   existing routing. Clicking empty chrome does not auto-background.
3. **Actual window focus loss with no modal or transient owner** — the only
   backgrounding candidate. One physical click must never both dismiss a
   modal AND background a session; when step 4 lands, a modal-dismissal epoch
   guard must prevent the parent window's delayed key restoration from being
   misread as a background-worthy blur.

### Where

`src/main_sections/render_impl.rs`, the `was_window_focused && !is_window_focused`
edge. That is the single choke point for "the user left" — there is no NSWindow
delegate; GPUI frame-polls `isKeyWindow`. The current outcome is binary:

```
ScriptList  -> hide_main_window_preserving_state_for_focus_loss()
everything  -> close_and_reset_window()          // destroys the view
```

It gains a third arm: **an AI surface with a backgroundable session backgrounds
instead of closing.**

### What must NOT change

Agent Chat's `DismissPolicy` is `explicit_cmd_w_only` — blur is `Ignore`. Two
places deliberately depend on that:

- `gesture_routing.rs`: "an embedded Agent Chat is a sticky surface that
  survives click-outside… reclaim key + composer focus instead of destroying
  the live session";
- `window_visibility.rs`: "an implicit hide must not destroy a live Agent Chat
  session — bring it back exactly as it was".

**Do not flip Agent Chat's policy to blur-closes.** Backgrounding is a strictly
better outcome than either current behaviour, but it must be reached by adding
the new arm, not by inverting a policy two other paths read.

### The modal must not trip the blur handler

Opening the parent action dialog is a separate NSPanel, so the main window loses
key status the instant the dialog appears. The blur handler is guarded by
`!confirm::is_confirm_window_open()`. **Any new modal must register with that
same predicate** or the blur path fires underneath it — and, given the arm above,
would background the session while the user is still deciding whether to.

---

## 4. The main-menu row

### Placement

Prepends in `filtering_cache.rs` are ordered, and later prepends outrank earlier
ones. Today: base results → alias pin → Brain Inbox → **Active Flows** →
calculator → attachment portal.

Backgrounded sessions go in **one prepend after Active Flows**, making them
first. Follow the existing shape exactly: shift `GroupedListItem::Item` indices,
insert rows at the flat front, splice a `SectionHeader` at grouped index 0.

### Merge with Active Flows, do not add a parallel section

"Active Flows" and "Backgrounded sessions" are the same idea seen twice. Two
adjacent sections that both mean "conversations you left running" is exactly the
inconsistency this whole pass is meant to remove. **One section, one header**
(proposed: **"Conversations"**), containing every live-or-backgrounded session
across all three surfaces, ordered by `last_activity` descending.

### Row type

Reuse `SearchResult::Flow(FlowMatch)` — it already carries `session_id`, and
Enter on it already resumes via `selection_fallback.rs`. Adding a new
`SearchResult` variant would mean touching ~12 call sites
(`is_selectable_result`, `stable_selection_key`, the source-bucket splits,
`focused_info`, `menu_syntax/filter`, `main_window_preflight/build`,
`prompt_and_script_list_collectors`, `designs/core/render`, plus Enter
dispatch). If `FlowMatch` genuinely cannot describe an Agent Chat session,
widen `FlowMatch` before adding a variant.

### The header always shows — but only when there is something to show

`prepend_root_flow_sessions_section` returns early with no header when nothing
is live. Keep that: the persistent-leading-separator rule (POLISH.md §2) is
about a section that *exists* not vanishing between frames, not about
manufacturing an empty one.

### Decide the suppression gates deliberately

Both existing prepends are wrapped in
`if !menu_syntax_owns_main_list && !spine_owns_for_computed`. A backgrounded
session will silently vanish in menu-syntax mode and whenever the spine owns the
list. **Proposed:** keep the suppression (those modes are explicitly
task-focused), but say so in the code rather than inheriting it by copy-paste.

---

## 5. The one product decision that must be renegotiated

**Quick AI is currently forbidden from ever resurrecting a transcript.**

- `AgentChatCapabilities::QUICK_AI` sets `retained_threads: false, history: false`.
- `embedded_cache_reuse_allowed` states: *"a closed Quick AI view must NEVER be
  reopened with its prior transcript/draft, even QuickAi→QuickAi"*.
- `evict_embedded_agent_chat_session` drops the entity and the warm Pi lease on
  close.

This is deliberate and tested. Backgrounding a Quick AI session reverses it.

**Proposed resolution — background Quick AI, but only until the window closes.**
Quick AI is the "fastest answer, then get out of my way" surface; a Quick AI
session that outlives an app restart is a different product. So:

- Quick AI sessions ARE backgroundable and DO appear in Conversations;
- they are memory-only and never persisted;
- they expire on app quit, and on an idle timeout (proposed: 30 minutes);
- `AgentChatSessionPolicy::admit_context` still adjudicates every resume, so a
  resumed Quick AI can never become a laundering path for `Full` context.

Flow and Agent Chat sessions persist as they do today.

**This needs an explicit yes.** It is the only part of this design that changes
what a surface *is*, rather than how it is dismissed.

---

## 6. Known gaps that block implementation

- **G1 — no activity timestamp.** `FlowSessionMeta.started_at` is a
  `std::time::Instant`: monotonic, non-serializable, and it records *start*, not
  *last use*. Ordering by recency needs a new field. Agent Chat's SQLite store
  already has `updated_at`; the in-memory registry needs to agree with it.
- **G2 — no persisted session identity.** `PersistedFlowConversation` is
  transcript-only, one snapshot per flow, with no session id. A backgrounded
  session's identity dies at app exit today.
- **G3 — RESOLVED 2026-07-25, and the original wording was wrong.** Backdrop
  clicks are NOT missing: `src/render_prompts/arg/helpers.rs:103–134` builds a
  real full-bleed backdrop behind the actions dialog and dismisses it on
  click. What is unreachable is only the *policy lookup* — the
  `DismissTrigger::BackdropClick` variant (`app_view_state.rs:694`, `:807`)
  is never constructed, while the behavior lives in a hardcoded handler that
  bypasses the policy table. Ruling (user, submission `98cab5e5-…641`):
  a backdrop click dismisses the modal only and is consumed; it never
  backgrounds a session. Do not revive the generic trigger as "background";
  either route the existing handler through a modal-scoped policy or leave
  the variant unused.
- **G4 — three parallel keyboard paths.** Per `flows/escape.md`: capture
  interceptors in `startup.rs`, per-surface bubble handlers, and the automation
  mirror in `simulate_key_dispatch.rs`. Automation probes only exercise the
  third, **so a partial fix looks green**. All three change together.
- **G5 — source-text audits.** `src/window_state/tests/window_state.rs` does
  literal source-text audits of `close_and_reset_window` and
  `hide_main_window_preserving_state_for_focus_loss` bodies. Adding a third blur
  arm will trip them. Per the Source Audit Test Policy, that is the third-strike
  signal to rewrite those structurally rather than patch their strings again.

---

## 7. Glass Motion Calibration Lock

`close_and_reset_window` runs `begin_main_window_exit_dematerialize()` and a
hard-coded **135 ms** deferred-hide delay matched to the locked popup removal
delay.

**Backgrounding must reuse the existing popup surface and its calibrated
entry/exit.** Do not introduce a new animated overlay, and do not "tidy" that
135 ms while adding the third arm. Retuning requires explicit user permission
per `CLAUDE.md`.

---

## 8. Build order

Each step is independently verifiable; none is useful alone, so they land in
this order.

1. **G1** — add `last_activity` to session meta and keep it current. Re-sort the
   existing Active Flows section by it. *Proof:* two sessions, return to the
   older one, it moves to the top.
2. **Registry** — wire `agent_task_dock.rs` as the single backgrounded-session
   store on app state. *Proof:* unit tests on the pure model, plus a receipt
   field in the driver state snapshot.
3. **Section merge** — one "Conversations" section replacing Active Flows,
   carrying all three surfaces. *Proof:* extend
   `active_flow_session_section_tests`; a probe asserting it sits at `flat[0]`
   above Brain Inbox.
4. **Blur arm** — the third outcome in the focus-loss handler. *Proof:* a
   devtools probe that opens each surface, clicks away, and asserts the session
   is still resumable from the main menu. This is the step that needs G5 resolved.
5. **Escape modal** — last rung only, with the skip rule. *Proof:* the escape
   ladder probe, extended per surface; all three keyboard paths updated together
   (G4).
6. **Quick AI** — only after an explicit yes on §5.

---

## 9. Anchors

Verified against the tree at the time of writing.

| Thing | Where |
|---|---|
| Session store | `src/main_sections/app_state.rs:1041` (`flow_sessions`) |
| Session meta | `src/flows/session.rs:405` (`started_at` is an `Instant`, `:423`) |
| Blur choke point | `src/main_sections/render_impl.rs:249` |
| Dismiss policy | `src/main_sections/app_view_state.rs:723` |
| Dead backdrop trigger | `src/main_sections/app_view_state.rs:694`, `:807` |
| Flow Escape → background | `src/render_builtins/flow_ux.rs:167`, `:1594` |
| Go-back vs close | `src/app_impl/lifecycle_reset.rs:926` |
| Main-menu prepend | `src/scripts/grouping.rs:332`, called from `src/app_impl/filtering_cache.rs:2028` |
| Enter resumes a session | `src/app_impl/.../selection_fallback.rs:591` |
| Three-button modal | `src/confirm/parent_dialog.rs:276` |
| Unwired prior art | `src/ai/agent_task_dock.rs` (only `src/ai/mod.rs:28`, `:59` reference it) |

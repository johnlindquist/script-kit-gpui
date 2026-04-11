# ACP Chat

ACP Chat is the canonical chat surface for ACP-compatible agents in Script Kit GPUI. The code still uses `tab_ai_*` names in a few places, but the current surface is the ACP chat view and its detached popup window.

## Entry paths

ACP opens through `open_tab_ai_acp_with_entry_intent(...)` and related launcher flows. If a detached ACP window is already open, the app focuses that window instead of opening a second one.

Plain `Tab` from the launcher also routes into ACP. If the detached chat window already exists and the launcher has text, the app submits that text into the detached thread; otherwise it opens ACP and stages the current launcher context there.

Some adjacent flows still route to `QuickTerminalView` instead of ACP when the task needs a PTY-backed harness surface. That boundary matters because `QuickTerminalView` is the verification-oriented terminal wrapper, not the chat UI.

## Detached window behavior

The detached ACP window lives in `src/ai/acp/chat_window.rs` and carries a live thread when opened from an existing conversation.

`open_chat_window_with_thread(...)` transfers a live `AcpThread` into that window, stores the handle for later focus/close operations, and registers a stable automation ID for runtime targeting.

The detached window wires the same core footer actions as the embedded view: toggle actions, close, and history. `Cmd+W` closes the detached popup directly from `AcpChatView`; the main panel handles the equivalent close gesture through the app-level interceptor.

Detached ACP also keeps the thread alive for reuse. When the window is already open, ACP entry focuses it rather than creating another copy of the chat surface.

## Context staging

ACP entry stages context in the current codebase, not just in old compatibility helpers. The launch path captures a UI snapshot, resolves desktop context, seeds the apply-back route, and then switches to ACP before deferred capture finishes.

The staged context can start from different inputs:

- a focused target chip, via `open_tab_ai_acp_with_explicit_target(...)`
- a selected plugin skill, via `open_acp_with_selected_skill(...)`
- the Ask Anything minimal desktop context resource
- an explicit ambient capture label for launcher-driven AI commands

For focused-target launches, the thread gets an inline token immediately and marks context bootstrap ready without waiting for deferred capture. For Ask Anything launches, ACP first stages the minimal context resource and then fills in the rest after the first paint.

If the launch is not ready yet, ACP renders an inline setup card instead of a broken chat surface. That setup path is part of the current `AcpChatView` contract.

## ACP composer

`src/ai/acp/view.rs` owns the composer, message rendering, inline mention parsing, slash picker, history popup, and portal callbacks.

The current implementation supports inline mention sessions, slash-command sessions, and the context preview / portal flow that replaces stale wiki-era “dead token” language.

The composer’s footer callbacks are host-driven. The view exposes hooks for toggle actions, close, and history so embedded ACP and detached ACP can share the same UI logic without borrowing the view at the wrong time.

## Boundary with `QuickTerminalView`

`QuickTerminalView` is a separate surface with different semantics.

- it is PTY-backed
- it is used for harness or verification-oriented flows
- `Tab` and `Shift+Tab` inside the terminal belong to the PTY, not to ACP focus navigation
- `Cmd+Enter` apply-back behavior is terminal-specific and uses the harness route logic

ACP Chat should not be described as that terminal surface. The current code treats them as related AI entry paths, not the same product surface.

## Current code references

These are the live files that define the ACP surface today.

- `src/app_impl/tab_ai_mode.rs` for ACP entry routing, detached-window reuse, and context staging
- `src/ai/acp/chat_window.rs` for detached window lifecycle and action wiring
- `src/ai/acp/view.rs` for the ACP composer, inline mentions, history, and portal behavior
- `src/main_sections/app_view_state.rs` for the `AppView` routing enum

## Stale claims corrected

These are the old wiki claims this page no longer repeats.

- ACP Chat is not the only AI surface in the app; `QuickTerminalView` still exists for PTY-backed harness flows.
- Plain `Tab` does not always create a fresh ACP surface; if a detached chat window exists, the app focuses that window and may submit into it.
- Context staging is not just a single compatibility helper anymore; it now includes focused targets, explicit skill handoffs, and deferred desktop capture.
- Detached ACP is not just an embedded panel detail; it is a separate popup window with its own focus, close, and history behavior.

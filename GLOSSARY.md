# Script Kit GPUI UI Glossary & Code Map

This document defines the main user-facing UI surfaces and components in Script Kit GPUI and maps them to their respective locations in the source code.

---

## App-independent domain crates

- **ASCII and fuzzy launcher matching:**
  [crates/sk-protocol/src/ascii_search.rs](crates/sk-protocol/src/ascii_search.rs)
  owns allocation-free ASCII case folding, word boundaries, exact names,
  ordered fuzzy matches, original highlight indices, and short-query policy.
  [src/scripts/search/ascii.rs](src/scripts/search/ascii.rs) preserves all
  launcher, browser-tab, spine, script, and metadata imports as a facade.
- **Structured launcher queries and category routing:**
  [crates/sk-protocol/src/query_prefix.rs](crates/sk-protocol/src/query_prefix.rs)
  owns `tag:`, `author:`, `kit:`, `is:`, `type:`, `group:`, and `tool:`
  parsing plus deterministic built-in, app, window, flow, skill, script, and
  scriptlet category routing.
  [src/scripts/search/prefix_filters.rs](src/scripts/search/prefix_filters.rs)
  preserves every launcher import and retains application-owned script and
  scriptlet metadata matching.
- **Cross-provider search thresholds, scores, paths, and highlights:**
  [crates/sk-protocol/src/search_primitives.rs](crates/sk-protocol/src/search_primitives.rs)
  owns Unicode-aware query thresholds, deterministic saturating rank scores,
  private script/scriptlet display paths, and allocation-free, fail-closed
  highlight boundaries. [src/scripts/search/match_contract.rs](src/scripts/search/match_contract.rs)
  and [src/scripts/search/paths.rs](src/scripts/search/paths.rs) preserve
  existing launcher, Notes, Clipboard, Dictation, Agent Chat, and AI Vault
  imports as compatibility adapters.
- **Search/provider ownership:**
  [crates/sk-protocol/src/search_contract.rs](crates/sk-protocol/src/search_contract.rs)
  owns deterministic search snapshots, exact provider generations, bounded
  source-owned refresh lifecycles, and root-launcher request coordination.
  [src/scripts/root_search_contract.rs](src/scripts/root_search_contract.rs)
  remains the application adapter and compatibility path; focused provider
  tests run in `sk-protocol` without GPUI, Metal, Whisper, or ONNX.
- **Natural-language sentence search:**
  [crates/sk-protocol/src/sentence_search.rs](crates/sk-protocol/src/sentence_search.rs)
  owns Unicode-aware query compilation, complete-word/prefix matching,
  low-information proximity, visible-versus-hidden ranking, and truthful
  highlight evidence for Clipboard, Dictation, conversations, and launcher
  rows. [src/scripts/search/sentence.rs](src/scripts/search/sentence.rs)
  preserves every existing app import as a compatibility facade.
- **Launcher filter coalescing:**
  [crates/sk-protocol/src/filter_coalescer.rs](crates/sk-protocol/src/filter_coalescer.rs)
  owns latest-query batching, single-worker scheduling, empty-query updates,
  and stale-work reset. [src/filter_coalescer.rs](src/filter_coalescer.rs)
  preserves the binary's existing import path; its behavior tests now run in
  `sk-protocol` rather than the disabled application-binary test target.
- **Private atomic storage:** [crates/sk-storage/src/lib.rs](crates/sk-storage/src/lib.rs)
  owns durable atomic writes, owner-only file/directory permissions, no-follow
  targets, collision-safe exports, and private JSONL boundaries. The existing
  [src/atomic_file.rs](src/atomic_file.rs) module is a compatibility facade.
  Focused `sk-storage` tests never compile or link GPUI, Metal, Whisper, or ONNX.

---

## 1. Core Windows & Presentation Modes

| UI Element | Description | Key Structs / Entities | Main Source File |
| :--- | :--- | :--- | :--- |
| **Script List** | The default launcher list view showing all scripts, recent items, and favorites when no prompts are active. Root Windows, file, and Brain search lifecycle state lives in a surface-owned store. | `ScriptListApp`, `RootSearchStore` | [render_impl.rs](src/main_sections/render_impl.rs), [app_state.rs](src/main_sections/app_state.rs), and [root_search_store.rs](src/main_sections/root_search_store.rs) |
| **Expanded View** | Main window presentation mode (`MainWindowMode::Full`) that expands the list area to show preview details or prompt shells. | `MainWindowMode` | [app_view_state.rs](src/main_sections/app_view_state.rs#L1361) |
| **Mini View** | Main window presentation mode (`MainWindowMode::Mini`) that uses a single-column layout for quick selection. | `MainWindowMode` | [app_view_state.rs](src/main_sections/app_view_state.rs#L1361) |
| **Notes Window** | Floating, persistent, editor-only secondary window for creating and browsing notes. Cmd+P uses the shared Notes search container and can open regular notes or day notes inside the Notes editor. Ask AI / Cmd+Enter opens (or activates) the MAIN window's Agent Chat with the current live note snapshot staged as explicit `@note` context — the Notes window itself hosts no Agent Chat surface and always stays open. | `NotesApp`, `handoff_selected_note_to_main_agent_chat` | [window.rs](src/notes/window.rs) & [ai_handoff.rs](src/notes/window/ai_handoff.rs) |
| **Dictation Window** | overlay panel with one anatomy across all phases: header row (timer, icon destination verb chips Paste · Today · Ask · Send, target badge), a wrapped multi-line caption block that reveals transcript words one at a time (`live_caption.rs`) and grows the window bottom-anchored as text accumulates, and the native footer rail. Processing phases keep the layout — grayed timer/badge, status label, pulsing caption, real finalize progress bar (chunked long-audio transcription). | `DictationOverlay` | [window.rs](src/dictation/window.rs#L503) |
| **Main Input** | The top search text box where users type filter queries. | `gpui_input_state` (`TextInputState`) | [text_input.rs](src/components/text_input.rs) & [common.rs](src/render_builtins/common.rs#L50) |
| **Footer** | Visually detached native hint capsules below an 8-point transparent gutter. On Tahoe the footer and bounded GPUI main stage are siblings inside the same main `NSWindow`, so live window drags move them in one compositor transaction; reusable secondary-window footers keep their own lifecycle. | `MainWindowFooterConfig`, `MainWindowDetachedFooterRegions` | [footer_popup.rs](src/footer_popup.rs) |

---

## 2. Popups & Dialogs

| UI Element | Description | Key Structs / Entities | Main Source File |
| :--- | :--- | :--- | :--- |
| **Actions Menu** | Searchable, categorised contextual operations menu shown as a popover overlay (Cmd+K). | `ActionsDialog` | [dialog.rs](src/actions/dialog.rs#L520) & [window.rs](src/actions/window.rs) |
| **Trigger Picker** | Main-list picker rows suggesting capture targets and handlers when prefix characters are typed (e.g. `;`, `+`, `:`). | `MenuSyntaxTriggerPickerState` | [menu_syntax_trigger_picker_main_list.rs](src/app_impl/menu_syntax_trigger_picker_main_list.rs) & [menu_syntax_trigger_picker.rs](src/app_impl/menu_syntax_trigger_picker.rs) |
| **Confirm Popup** | Dialog box overlay with customizable buttons (e.g. Yes/No/Cancel). | `ConfirmPopup` | [confirm/mod.rs](src/confirm/mod.rs) |

---

## 3. Interactive Script Prompts

These represent the interactive surfaces that scripts spawn when calling methods from the SDK (e.g., `arg()`, `div()`, `editor()`).

| UI Element | Description | Key Structs / Entities | Main Source File |
| :--- | :--- | :--- | :--- |
| **Arg Prompt** | Simple input fields prompting for single arguments. | `ArgPrompt` / `render_arg_prompt` | [render.rs](src/render_prompts/arg/render.rs) & [arg.rs](src/render_prompts/arg.rs) |
| **Chat Prompt** | AI agent chat surface (Agent Chat Portal) supporting streaming and prompt-specific layouts. | `ChatPrompt` / `render_chat_prompt` | [other.rs](src/render_prompts/other.rs#L441) |
| **Editor Prompt** | Rich multi-line text editor interface. | `EditorPrompt` / `render_editor_prompt` | [editor.rs](src/render_prompts/editor.rs) |
| **Form Prompt** | Prompts containing multiple custom input fields. | `FormPrompt` / `render_form_prompt` | [form.rs](src/render_prompts/form.rs) |
| **Select Prompt** | Dropdown menu allowing search and selection from a list of options. | `SelectPrompt` / `render_select_prompt` | [other.rs](src/render_prompts/other.rs) |
| **Div Prompt** | Custom HTML-like rendering surface controlled by the script. | `DivPrompt` / `render_div_prompt` | [div.rs](src/render_prompts/div.rs) |
| **Terminal Prompt** | Embedded terminal shell/PTY widget running executions. | `TermPrompt` / `render_term_prompt` | [term.rs](src/render_prompts/term.rs) & [term_prompt/mod.rs](src/term_prompt/mod.rs) |

---

## 4. Built-in Surfaces

Searchable utility lists available directly from the launcher.

| UI Element | Description | Key Structs / Entities | Main Source File |
| :--- | :--- | :--- | :--- |
| **Clipboard History** | Searchable history of clipboard entries. | `ClipboardHistoryView` | [clipboard.rs](src/render_builtins/clipboard.rs) & [clipboard_history/mod.rs](src/clipboard_history/mod.rs) |
| **Emoji Picker** | Panel for searching and inserting emojis. | `EmojiPickerView` | [emoji_picker.rs](src/render_builtins/emoji_picker.rs) |
| **Process Manager** | Search tool to view and kill system processes. | `ProcessManager` | [process_manager.rs](src/render_builtins/process_manager.rs) |
| **Window Switcher** | Switch focus between active application windows. | `WindowSwitcher` | [window_switcher.rs](src/render_builtins/window_switcher.rs) |
| **App Launcher** | Search and launch installed local applications. | `AppLauncher` | [app_launcher.rs](src/render_builtins/app_launcher.rs) |
| **Notes Browse** | List and search local Markdown notes. | `NotesBrowseView` | [notes_browse.rs](src/render_builtins/notes_browse.rs) |
| **File Search** | Browse files on the local filesystem. | `FileSearchView` | [file_search.rs](src/render_builtins/file_search.rs) |
| **Permissions Wizard** | Guided grant flow for the macOS permissions Script Kit needs (Accessibility, Screen Recording, Event Synthesizing, Input Monitoring, Microphone) with live TCC status cards. Opens on fresh installs and via "Set Up Permissions". | `PermissionsWizardView` | [permissions_wizard.rs](src/render_builtins/permissions_wizard.rs) & [permissions_wizard.rs](src/permissions_wizard.rs) |

---

## 5. Memory Layer (Script Kit Brain)

| UI Element | Description | Key Structs / Entities | Main Source File |
| :--- | :--- | :--- | :--- |
| **Day Page** | Today's diary surface inside the main launcher window — same window frame as Script List, hosts the shared notes editor and defaults to `brain/days/<today>.md`. Cmd+P uses the same Notes search container/result language as the Notes Window, but selections open locally in the Day Page editor unless the explicit "Open in Notes Window" action is run. | `DayPageView`, `AppView::DayPage` | [day_page_view.rs](src/main_sections/day_page_view.rs) & [day_page_types.rs](src/main_sections/day_page_types.rs) |
| **Script Kit Brain substrate** | Canonical markdown memory under `~/.scriptkit/brain/{days,fragments,notes,trash}` — day-page append API, fragment writer, atomic writes, trash/restore. SQLite indexes are derived only. | `BrainSubstrate`, `DayEntry` | [substrate/mod.rs](src/brain/substrate/mod.rs) |
| **Gesture classifier** | Pure state machine classifying main-hotkey key-down/key-up into tap, hold, double-tap, and key-down instant show. Wired into main-window surface morphs. | `GestureClassifier`, `GestureEvent` | [gesture.rs](src/hotkeys/gesture.rs) & [gesture_routing.rs](src/main_sections/gesture_routing.rs) |
| **Fragment** | Long captures (>200 words) stored as `brain/fragments/<date>-<HHMM>-<source-slug>.md` with provenance frontmatter; the day page references them via excerpt + relative link. | `BrainSubstrate::write_fragment` | [fragment.rs](src/brain/substrate/fragment.rs) |
| **Sediment** | Clipboard auto-keep: URLs land on today's day page; non-URLs promote on re-copy (`copy_count ≥ 2`). Day Page renders kept-URL links and fragment excerpt cards. | `ClipboardSedimentTier`, `DayPageSegment` | [sediment.rs (clipboard)](src/clipboard_history/sediment.rs) & [sediment.rs (day page)](src/day_page/sediment.rs) |
| **Post-copy tracker** | Clipboard copies flow through sediment rules without opening popup UI. URLs auto-keep to the Day Page and non-URLs promote on re-copy. | `process_text_sediment` | [sediment.rs](src/clipboard_history/sediment.rs) |

---

## 6. Flow Launcher (mdflow)

| UI Element | Description | Key Structs / Entities | Main Source File |
| :--- | :--- | :--- | :--- |
| **Flow Desk (Conversation Desk)** | The flow-first surface (main-window "Flows" builtin): sessions, background runs, and the flow corpus in one list. Enter on a flow = converse (Threadline session; codex flows use a persistent app-server thread, other engines run one `md --events` turn per message); ⇧↵ = run once in the background; interactive (TTY-only) flows open in the Quick Terminal; Esc backgrounds/goes back (never cancels). Onboarding rows appear when mdflow is missing (Install) or the roster is empty (`md init`). Contract: `docs/ai/flow-ux-protocol.md`. | `AppView::FlowUxView`, `FlowDeskRow` | [flow_ux.rs](src/render_builtins/flow_ux.rs) |
| **Flow sessions (Threadline)** | Per-flow conversations with uncapped-by-app v4 thread persistence keyed by flow id + definition path, so same-named flows in different projects never collide. ⌘L New Conversation archives the current thread and starts a blank active thread; archived threads are retained/read-only and Continue as New clones one into a writable child while preserving lineage. ⌘K exposes Stop (turn only), Conversation History, Copy Last Response/Transcript, confirmed Delete Conversation, and confirmed Actions-only Terminate Runtime. Terminate has no shortcut: it settles active work, forgets only runtime state, and preserves transcripts, archives, drafts, and persistence. Delete removes only the selected thread behind confirmation and a tombstone fence. Back, Background, Escape, Close, and Cmd+W never delete. Every advertised chord is held against `resolve_flow_session_key_action` by `every_advertised_session_shortcut_has_a_declared_owner`. Answer text renders through the SHARED selectable conversation renderer, not Flow's own markdown engine. | `FlowSessionMeta`, `FlowTranscriptSelection`, `AppView::FlowSessionView` | [session.rs](src/flows/session.rs) |
| **Shared conversation renderer** | The ONE owner of how an AI answer looks and behaves across Agent Chat and Flow: paint values (`conversation_style.rs`), the selectable `TextView` seam (`conversation_text.rs`), the per-turn copy control (`conversation_actions.rs`), and the jump-to-latest pill (`list_scroll_affordance.rs`). `agent_chat/ui/style_contract.rs` is a façade over it with zero production values. A turn's render identity is `ConversationTurnRenderKey`, resolved once from its ORIGINATING message — NOT `message_id`, which moves to the assistant's id on first reply. | `ConversationStyleDef`, `conversation_markdown_view`, `ConversationTurnRenderKey` | [conversation_style.rs](src/components/conversation_style.rs) & [conversation_text.rs](src/components/conversation_text.rs) |
| **Flow runs (in-desk supervision)** | Background run-once/workflow runs are desk rows (phase · elapsed · last output) with ⌘K Cancel Run / Copy Output / Clear Finished, plus exactly one completion/failure toast per run. The detached Flow Manager window and the Flash/Dispatch/Lens/Mission-Control UX variants are dead (`BuiltInFeature::FlowManager` aliases to the desk). | `FlowDeskRow::Run`, `FlowRunRegistry` | [flow_ux.rs](src/render_builtins/flow_ux.rs) |
| **Flow run substrate** | mdflow owns discovery/execution (`md roster --json`, `md <flow> --events`); the app consumes the event stream STRICTLY (protocol-version check, gapless seq, ordering state machine — violations fail the run closed and kill the process; a missing exit code is never success). Cancellation is truthful: `Cancelling` → SIGTERM → 2s → SIGKILL on the process group → `Cancelled` only once the group is confirmed dead. Raw child stderr is captured as diagnostics; conversation turns stream from an append-only capture, never the bounded display tail. | `FlowRunRegistry`, `EventStreamValidator`, `launch_flow` | [run_registry.rs](src/flows/run_registry.rs) & [runner.rs](src/flows/runner.rs) |

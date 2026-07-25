# Script Kit GPUI UI Glossary & Code Map

This document defines the main user-facing UI surfaces and components in Script Kit GPUI and maps them to their respective locations in the source code.

---

## 1. Core Windows & Presentation Modes

| UI Element | Description | Key Structs / Entities | Main Source File |
| :--- | :--- | :--- | :--- |
| **Script List** | The default launcher list view showing all scripts, recent items, and favorites when no prompts are active. Root Windows, file, and Brain search lifecycle state lives in a surface-owned store. | `ScriptListApp`, `RootSearchStore` | [render_impl.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/main_sections/render_impl.rs), [app_state.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/main_sections/app_state.rs), and [root_search_store.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/main_sections/root_search_store.rs) |
| **Expanded View** | Main window presentation mode (`MainWindowMode::Full`) that expands the list area to show preview details or prompt shells. | `MainWindowMode` | [app_view_state.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/main_sections/app_view_state.rs#L1361) |
| **Mini View** | Main window presentation mode (`MainWindowMode::Mini`) that uses a single-column layout for quick selection. | `MainWindowMode` | [app_view_state.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/main_sections/app_view_state.rs#L1361) |
| **Notes Window** | Floating, persistent overlay editor panel for creating and browsing notes. Cmd+P uses the shared Notes search container and can open regular notes or day notes inside the Notes editor. | `NotesApp` | [window.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/notes/window.rs#L246) |
| **Dictation Window** | overlay panel with one anatomy across all phases: header row (timer, icon destination verb chips Paste · Today · Ask · Send, target badge), a wrapped multi-line caption block that reveals transcript words one at a time (`live_caption.rs`) and grows the window bottom-anchored as text accumulates, and the native footer rail. Processing phases keep the layout — grayed timer/badge, status label, pulsing caption, real finalize progress bar (chunked long-audio transcription). | `DictationOverlay` | [window.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/dictation/window.rs#L503) |
| **Main Input** | The top search text box where users type filter queries. | `gpui_input_state` (`TextInputState`) | [text_input.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/components/text_input.rs) & [common.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_builtins/common.rs#L50) |
| **Footer** | Visually detached native hint capsules below an 8-point transparent gutter. On Tahoe the footer and bounded GPUI main stage are siblings inside the same main `NSWindow`, so live window drags move them in one compositor transaction; reusable secondary-window footers keep their own lifecycle. | `MainWindowFooterConfig`, `MainWindowDetachedFooterRegions` | [footer_popup.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/footer_popup.rs) |

---

## 2. Popups & Dialogs

| UI Element | Description | Key Structs / Entities | Main Source File |
| :--- | :--- | :--- | :--- |
| **Actions Menu** | Searchable, categorised contextual operations menu shown as a popover overlay (Cmd+K). | `ActionsDialog` | [dialog.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/actions/dialog.rs#L520) & [window.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/actions/window.rs) |
| **Trigger Picker** | Main-list picker rows suggesting capture targets and handlers when prefix characters are typed (e.g. `;`, `+`, `:`). | `MenuSyntaxTriggerPickerState` | [menu_syntax_trigger_picker_main_list.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/app_impl/menu_syntax_trigger_picker_main_list.rs) & [menu_syntax_trigger_picker.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/app_impl/menu_syntax_trigger_picker.rs) |
| **Confirm Popup** | Dialog box overlay with customizable buttons (e.g. Yes/No/Cancel). | `ConfirmPopup` | [confirm/mod.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/confirm/mod.rs) |

---

## 3. Interactive Script Prompts

These represent the interactive surfaces that scripts spawn when calling methods from the SDK (e.g., `arg()`, `div()`, `editor()`).

| UI Element | Description | Key Structs / Entities | Main Source File |
| :--- | :--- | :--- | :--- |
| **Arg Prompt** | Simple input fields prompting for single arguments. | `ArgPrompt` / `render_arg_prompt` | [render.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_prompts/arg/render.rs) & [arg.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_prompts/arg.rs) |
| **Chat Prompt** | AI agent chat surface (Agent Chat Portal) supporting streaming and prompt-specific layouts. | `ChatPrompt` / `render_chat_prompt` | [other.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_prompts/other.rs#L441) |
| **Editor Prompt** | Rich multi-line text editor interface. | `EditorPrompt` / `render_editor_prompt` | [editor.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_prompts/editor.rs) |
| **Form Prompt** | Prompts containing multiple custom input fields. | `FormPrompt` / `render_form_prompt` | [form.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_prompts/form.rs) |
| **Select Prompt** | Dropdown menu allowing search and selection from a list of options. | `SelectPrompt` / `render_select_prompt` | [other.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_prompts/other.rs) |
| **Div Prompt** | Custom HTML-like rendering surface controlled by the script. | `DivPrompt` / `render_div_prompt` | [div.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_prompts/div.rs) |
| **Terminal Prompt** | Embedded terminal shell/PTY widget running executions. | `TermPrompt` / `render_term_prompt` | [term.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_prompts/term.rs) & [term_prompt/mod.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/term_prompt/mod.rs) |

---

## 4. Built-in Surfaces

Searchable utility lists available directly from the launcher.

| UI Element | Description | Key Structs / Entities | Main Source File |
| :--- | :--- | :--- | :--- |
| **Clipboard History** | Searchable history of clipboard entries. | `ClipboardHistoryView` | [clipboard.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_builtins/clipboard.rs) & [clipboard_history/mod.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/clipboard_history/mod.rs) |
| **Emoji Picker** | Panel for searching and inserting emojis. | `EmojiPickerView` | [emoji_picker.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_builtins/emoji_picker.rs) |
| **Process Manager** | Search tool to view and kill system processes. | `ProcessManager` | [process_manager.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_builtins/process_manager.rs) |
| **Window Switcher** | Switch focus between active application windows. | `WindowSwitcher` | [window_switcher.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_builtins/window_switcher.rs) |
| **App Launcher** | Search and launch installed local applications. | `AppLauncher` | [app_launcher.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_builtins/app_launcher.rs) |
| **Notes Browse** | List and search local Markdown notes. | `NotesBrowseView` | [notes_browse.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/notes_browse.rs) |
| **File Search** | Browse files on the local filesystem. | `FileSearchView` | [file_search.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_builtins/file_search.rs) |
| **Permissions Wizard** | Guided grant flow for the macOS permissions Script Kit needs (Accessibility, Screen Recording, Event Synthesizing, Input Monitoring, Microphone) with live TCC status cards. Opens on fresh installs and via "Set Up Permissions". | `PermissionsWizardView` | [permissions_wizard.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_builtins/permissions_wizard.rs) & [permissions_wizard.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/permissions_wizard.rs) |

---

## 5. Memory Layer (Script Kit Brain)

| UI Element | Description | Key Structs / Entities | Main Source File |
| :--- | :--- | :--- | :--- |
| **Day Page** | Today's diary surface inside the main launcher window — same window frame as Script List, hosts the shared notes editor and defaults to `brain/days/<today>.md`. Cmd+P uses the same Notes search container/result language as the Notes Window, but selections open locally in the Day Page editor unless the explicit "Open in Notes Window" action is run. | `DayPageView`, `AppView::DayPage` | [day_page_view.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/main_sections/day_page_view.rs) & [day_page_types.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/main_sections/day_page_types.rs) |
| **Script Kit Brain substrate** | Canonical markdown memory under `~/.scriptkit/brain/{days,fragments,notes,trash}` — day-page append API, fragment writer, atomic writes, trash/restore. SQLite indexes are derived only. | `BrainSubstrate`, `DayEntry` | [substrate/mod.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/brain/substrate/mod.rs) |
| **Gesture classifier** | Pure state machine classifying main-hotkey key-down/key-up into tap, hold, double-tap, and key-down instant show. Wired into main-window surface morphs. | `GestureClassifier`, `GestureEvent` | [gesture.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/hotkeys/gesture.rs) & [gesture_routing.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/main_sections/gesture_routing.rs) |
| **Fragment** | Long captures (>200 words) stored as `brain/fragments/<date>-<HHMM>-<source-slug>.md` with provenance frontmatter; the day page references them via excerpt + relative link. | `BrainSubstrate::write_fragment` | [fragment.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/brain/substrate/fragment.rs) |
| **Sediment** | Clipboard auto-keep: URLs land on today's day page; non-URLs promote on re-copy (`copy_count ≥ 2`). Day Page renders kept-URL links and fragment excerpt cards. | `ClipboardSedimentTier`, `DayPageSegment` | [sediment.rs (clipboard)](file:///Users/johnlindquist/dev/script-kit-gpui/src/clipboard_history/sediment.rs) & [sediment.rs (day page)](file:///Users/johnlindquist/dev/script-kit-gpui/src/day_page/sediment.rs) |
| **Post-copy tracker** | Clipboard copies flow through sediment rules without opening popup UI. URLs auto-keep to the Day Page and non-URLs promote on re-copy. | `process_text_sediment` | [sediment.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/clipboard_history/sediment.rs) |

---

## 6. Flow Launcher (mdflow)

| UI Element | Description | Key Structs / Entities | Main Source File |
| :--- | :--- | :--- | :--- |
| **Flow Desk (Conversation Desk)** | The flow-first surface (main-window "Flows" builtin): sessions, background runs, and the flow corpus in one list. Enter on a flow = converse (Threadline session; codex flows use a persistent app-server thread, other engines run one `md --events` turn per message); ⇧↵ = run once in the background; interactive (TTY-only) flows open in the Quick Terminal; Esc backgrounds/goes back (never cancels). Onboarding rows appear when mdflow is missing (Install) or the roster is empty (`md init`). Contract: `docs/ai/flow-ux-protocol.md`. | `AppView::FlowUxView`, `FlowDeskRow` | [flow_ux.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_builtins/flow_ux.rs) |
| **Flow sessions (Threadline)** | Per-flow conversations with streaming turns, persisted transcripts (keyed by flow id + definition path so same-named flows in different projects never collide), reattach-on-restart, stale-definition re-thread, ⌘K Stop (turn only), ⌘L New Conversation (discards the transcript, keeps the session — refused with a neutral toast while a turn is in flight, and withheld from the ⌘K list entirely while working), and ⇧⌘⎋ Terminate (ends the conversation AND erases its persisted history). Answer text renders through the SHARED selectable conversation renderer, not Flow's own markdown engine. | `FlowSessionMeta`, `AppView::FlowSessionView` | [session.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/flows/session.rs) |
| **Shared conversation renderer** | The ONE owner of how an AI answer looks and behaves across Agent Chat and Flow: paint values (`conversation_style.rs`), the selectable `TextView` seam (`conversation_text.rs`), the per-turn copy control (`conversation_actions.rs`), and the jump-to-latest pill (`list_scroll_affordance.rs`). `agent_chat/ui/style_contract.rs` is a façade over it with zero production values. A turn's render identity is `ConversationTurnRenderKey`, resolved once from its ORIGINATING message — NOT `message_id`, which moves to the assistant's id on first reply. | `ConversationStyleDef`, `conversation_markdown_view`, `ConversationTurnRenderKey` | [conversation_style.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/components/conversation_style.rs) & [conversation_text.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/components/conversation_text.rs) |
| **Flow runs (in-desk supervision)** | Background run-once/workflow runs are desk rows (phase · elapsed · last output) with ⌘K Cancel Run / Copy Output / Clear Finished, plus exactly one completion/failure toast per run. The detached Flow Manager window and the Flash/Dispatch/Lens/Mission-Control UX variants are dead (`BuiltInFeature::FlowManager` aliases to the desk). | `FlowDeskRow::Run`, `FlowRunRegistry` | [flow_ux.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/render_builtins/flow_ux.rs) |
| **Flow run substrate** | mdflow owns discovery/execution (`md roster --json`, `md <flow> --events`); the app consumes the event stream STRICTLY (protocol-version check, gapless seq, ordering state machine — violations fail the run closed and kill the process; a missing exit code is never success). Cancellation is truthful: `Cancelling` → SIGTERM → 2s → SIGKILL on the process group → `Cancelled` only once the group is confirmed dead. Raw child stderr is captured as diagnostics; conversation turns stream from an append-only capture, never the bounded display tail. | `FlowRunRegistry`, `EventStreamValidator`, `launch_flow` | [run_registry.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/flows/run_registry.rs) & [runner.rs](file:///Users/johnlindquist/dev/script-kit-gpui/src/flows/runner.rs) |

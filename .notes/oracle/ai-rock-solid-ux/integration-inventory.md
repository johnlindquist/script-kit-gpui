# S00 AI reliability integration inventory

This is the frozen current-checkout owner map for Oracle Execute step S00. It is evidence, not permission to broaden beyond AI terminal outcome/recovery paths. All source paths listed in `preexisting-dirty-paths.txt` are excluded from edits and commits.

## Routing/process receipt

- `flows/devtools.md`: **Codex isolation preparation failed before engine start; direct fallback authorized.**
- `flows/agent-chat.md`: **Codex isolation preparation failed before engine start; direct fallback authorized.**
- Per Oracle S00, do not retry those flow calls during this execution.

## Pure-domain candidate and dependencies

| Owner | Current evidence | S01/S02 disposition |
|---|---|---|
| `crates/sk-protocol/Cargo.toml` | App-independent crate with no current dependencies. | Own the pure typed reliability state machine; any new dependency must remain app-independent. |
| `crates/sk-protocol/src/lib.rs` | Only exposes semantic-id primitives today. | Export `ai_reliability`; exhaustive types/reducer/tests live in this crate. |
| `src/ai/mod.rs` | App AI composition root. | Export app-side classifier, diagnostics, capability, and presentation adapters without leaking GPUI/effects into `sk-protocol`. |

## Agent Chat and Pi event emitters/adapters

| Concern | Exact owners | Current terminal shape |
|---|---|---|
| Shared runtime protocol | `src/ai/agent_chat/events.rs`, `src/ai/agent_chat/runtime.rs` | `AgentChatEvent::Failed { error: String }`. |
| Pi protocol/runtime | `src/ai/agent_chat/pi/events.rs`, `src/ai/agent_chat/pi/runtime.rs`, `src/ai/agent_chat/pi/protocol.rs`, `src/ai/agent_chat/pi/auth_recovery.rs` | Parses/emits string failures; separate auth-launch effect. |
| Codex exec runtime | `src/ai/agent_chat/codex_exec.rs` | Parses JSONL, applies Quick AI web-search policy, emits/returns internal string errors. |
| Thread state | `src/ai/agent_chat/ui/thread.rs`, `src/ai/agent_chat/ui/thread/tests.rs` | `AgentChatThreadStatus::Error`; local string classifier; callout with `raw_detail`, retry/auth flags. |
| View/effects | `src/ai/agent_chat/ui/view.rs`, `src/ai/agent_chat/ui/view/tests.rs`, `src/ai/agent_chat/ui/events.rs` | Local retry/auth/copy-error actions and renderer. |
| Warm/setup | `src/ai/agent_chat/agent_chat_recovery.rs`, `src/ai/agent_chat/warm_session.rs`, `src/ai/agent_chat/ui/preflight.rs`, `src/ai/agent_chat/ui/setup_state.rs` | Separate warm failure kind/state and setup cards. |
| Transcript | `src/ai/agent_chat/ui/components/transcript.rs` | Renders string error bubble and “saved in transcript” copy. |

## Quick AI

| Concern | Exact owners | Current evidence |
|---|---|---|
| Purpose/model/tools | `src/ai/agent_chat/profiles.rs`, `src/ai/agent_chat/launch.rs`, `src/ai/agent_chat/ui/ui_variant.rs` | Spark model, web-search-only, zero-context/ephemeral policy. |
| Codex enforcement | `src/ai/agent_chat/codex_exec.rs::apply_web_search` | Third distinct search returns `quick_ai_more_than_two_search_queries`. |
| Pi enforcement | `src/ai/agent_chat/profiles.rs`, launch argv builders | System prompt asks one search, runtime exposes only `web_search`. |
| Presentation | Agent Chat thread/view/transcript owners above | Internal policy identifier becomes primary failure copy. |
| Verification | `scripts/agentic/quick-ai-policy-probe.ts`, `scripts/agentic/quick-ai-fastest-search-probe.ts`, `scripts/agentic/agent-chat-web-search-benchmark.ts`, corresponding Bun tests | Proves model/tool/perf/current fail-closed contracts; not recovery UX. |

## Flow conversation and run

| Concern | Exact owners | Current terminal shape |
|---|---|---|
| Conversation model/persistence | `src/flows/session.rs` | `SessionTurn.error: Option<String>`, `needs_rethread`, conversation store. |
| Codex thread transport | `src/flows/codex_client.rs` | String error terminal events/session failures. |
| mdflow run protocol | `src/flows/runner.rs`, `src/flows/run_registry.rs`, `src/flows/model.rs` | Strict event validator and registry `error_message` string. |
| App/UI adapter | `src/render_builtins/flow_ux.rs` | Converts terminal strings to `FlowTurnOutcome::Failed`, then `ChatPrompt::set_message_error`. |
| Automation | `src/flows/automation.rs`, `scripts/agentic/flow-identity-row-reattach-probe.ts`, `scripts/agentic/flow-main-menu-reattach-probe.ts` | Flow state exists but no shared recovery category/action projection. |

## Legacy ChatPrompt / inline Mini AI

| Concern | Exact owners | Current terminal shape |
|---|---|---|
| Error taxonomy | `src/prompts/chat/types.rs` | `ChatErrorType::from_error_string`; invalid model displays “Model unavailable. Using default model.” |
| Message state | `src/prompts/chat/state.rs`, `src/prompts/chat/streaming.rs` | `ChatPromptMessage.error: Option<String>` and direct provider string storage. |
| Renderer | `src/prompts/chat/render_turns.rs` | Normalized error row plus raw error detail inline and optional retry callback. |
| Host/actions | `src/prompts/chat/prompt.rs`, `src/prompts/chat/actions.rs`, `src/app_impl/prompt_ai.rs`, `src/app_impl/chat_actions.rs` | Inline Mini AI/SDK ChatPrompt and Flow transcript host callbacks. |
| Protocol | `src/protocol/types/chat.rs`, `src/protocol/message/constructors/general.rs`, `src/protocol/message/variants/prompts_media.rs`, `src/main_sections/prompt_messages.rs` | SDK messages carry optional string error. |

## Remaining AI integration families to close in S11

These are current owners even when the frozen search pattern did not find a terminal string. S11 must explicitly classify each row as migrated to the shared model, a non-terminal helper with no user-visible terminal outcome, or an intentional reviewed exception. “No search hit” is not proof of no integration.

| Family | Exact owners / entry points | Current relation |
|---|---|---|
| Focused Text | `src/ai/focused_text/**`, `src/app_impl/agent_handoff/focused_text_entry.rs` | Agent Chat profile/variant; must inherit shared recovery/preservation behavior. |
| Local LLM / ghost backend | `src/ai/local_llm/**` | Separate model download/runtime/subprocess errors; user-visible AI integration audit required. |
| Provider-backed ChatPrompt | `src/ai/providers.rs`, `src/ai/config.rs`, `src/ai/model.rs`, `src/ai/session.rs`, `src/ai/storage.rs`, `src/ai/subscriptions.rs` | Provider/config/session/storage support for legacy and SDK AI. |
| AI presets and SDK handlers | `src/ai/presets.rs`, `src/ai/sdk_handlers.rs`, `src/ai/script_generation.rs`, `src/ai/public_contract_tests.rs` | ChatPrompt and script-generation entry/result paths. |
| Agent handoff/task dock | `src/ai/agent_prompt_handoff.rs`, `src/ai/agent_task_dock.rs`, `src/ai/explicit_target_handoff.rs` | Transfer/launch surfaces; preserve request/context on recovery. |
| Context/attachment helpers | `src/ai/context_contract.rs`, `src/ai/context_mentions/**`, `src/ai/context_selector/**`, `src/ai/context_preview.rs`, `src/ai/message_parts.rs`, `src/ai/tab_context.rs` | Non-provider effects but preservation/admission failures may need typed mapping. |
| Harness/Quick Submit | `src/ai/harness/**` | AI execution/handoff utility; terminal outcomes must not bypass shared mapping. |
| Current-app automation memory | `src/ai/current_app_automation_memory/**` | Supporting context source; not a provider turn unless a user-visible terminal path is found. |
| Result cards | `src/ai/result_cards.rs` | Presentation helper; must not become a second recovery card system. |
| OCR AI entry | `src/clipboard_history/ocr.rs` | Frozen scan contains `error_message`; audit whether its user-visible AI failure belongs in the shared `Other` intent. |
| Script-error Agent Chat handoff | `src/prompt_handler/script_error_context.rs`, `scripts/agentic/script-error-agent-chat-handoff-probe.ts` | Handoff path; preserve original error/context on Agent Chat recovery. |

## Protocol / automation dispatcher and surface-contract owners

| Layer | Exact owners |
|---|---|
| App surface identity | `src/main_sections/app_view_state.rs` (`AppView`, `SurfaceKind`, semantic surface mapping). |
| Main automation dispatcher | `src/main_entry/runtime_stdin.rs`, `src/main_entry/runtime_stdin_match_tail.rs`, `src/stdin_commands/mod.rs`, `src/protocol/message/variants/query_ops.rs`, `src/protocol/message/constructors/query_ops.rs`. |
| Agent Chat state schema | `src/protocol/types/agent_chat_state.rs`, `src/protocol/mod.rs`, `scripts/devtools/agent_chat.ts`. |
| Generic state/elements/layout | `src/protocol/types/automation_surface.rs`, `src/protocol/types/automation_inspect.rs`, `src/protocol/types/automation_inspect_geometry.rs`, `scripts/devtools/driver.ts`, `scripts/devtools/elements.ts`, `scripts/devtools/layout.ts`, `scripts/devtools/focus.ts`, `scripts/devtools/keyboard.ts`. |
| Surface discovery/contracts | `scripts/devtools/surface.ts`, `scripts/devtools/surfaces.ts`, `scripts/devtools/investigate.ts`, `docs/ai/contracts/surface-contracts.json`, `docs/ai/contracts/current-view-transitions.json`. |
| Flow automation | `src/flows/automation.rs`, `src/render_builtins/flow_ux.rs::flow_ux_automation_snapshot`. |
| Tests | `src/protocol/types/tests/**`, `scripts/devtools/*.test.ts`, Agent Chat/Flow automation contract tests under `tests/**`. |

Observed gaps frozen by the S00 DevTools receipts:

- Agent Chat: missing direct `devtools.agent_chat.inspect`, timeline, composer, and turn-diff fields.
- ChatPrompt: missing direct prompt inspect/scroll/selection/safe-submit fields.
- Flow: `FlowUxView` was unknown to `investigate.ts` despite `SurfaceKind::FlowSession` existing in app source.

## Persistence owners

| State | Exact owners / invariant to preserve |
|---|---|
| Agent Chat transcript/draft/context | `src/ai/agent_chat/ui/thread.rs`, `composer_state.rs`, `history.rs`, `history_attachment.rs`, `surface_state.rs`, `chat_window.rs`; preserve thread/profile/cwd/context and retry draft fingerprints. |
| Agent Chat runtime/session | `src/ai/agent_chat/pi/runtime.rs`, `warm_session.rs`, `ui/preflight.rs`; reattach/reconnect must not replay a mutating turn. |
| Flow conversation | `src/flows/session.rs::conversation_store`, session metadata/turn snapshots; versioned migration required for typed safe failures. |
| Flow run | `src/flows/run_registry.rs`, `src/flows/model.rs`; explicit restart only, cancellation remains distinct. |
| ChatPrompt messages | `src/prompts/chat/state.rs`, `src/prompts/chat/types.rs`; host callbacks and messages must preserve draft/turn on recovery. |

## Existing recovery and reliability probes

- `scripts/agentic/agent-chat-auth-recovery-probe.ts`
- `scripts/agentic/agent-chat-retry-recovery-probe.ts`
- `scripts/agentic/quick-ai-policy-probe.ts`
- `scripts/agentic/quick-ai-fastest-search-probe.ts`
- `scripts/agentic/agent-chat-web-search-benchmark.ts`
- `scripts/agentic/agent-chat-web-search-benchmark.test.ts`
- `scripts/agentic/flow-identity-row-reattach-probe.ts`
- `scripts/agentic/flow-main-menu-reattach-probe.ts`
- `scripts/agentic/flow-session-escape-ladder-probe.ts`
- `scripts/agentic/agent-chat-flow-session-geometry-timeline-probe.ts`
- `scripts/agentic/script-error-agent-chat-handoff-probe.ts`

## Frozen exact-pattern scan

Command:

```text
rg -n 'AgentChatEvent::Failed|ChatErrorType|set_message_error|error_message|raw_detail|Copy Error|Turn failed|Model unavailable\. Using default model|mark_failed\(|stream_message\(|SessionFailed|quick_ai_more_than_two_search_queries' src tests scripts
```

Every current result follows. Generic non-AI matches are retained as negative evidence; later steps may not silently infer they are in scope.

```text
src/protocol/message/constructors/query_ops.rs:234:            error_message: error_data.error_message,
src/protocol/message/constructors/query_ops.rs:245:    pub fn script_error(error_message: String, script_path: String) -> Self {
src/protocol/message/constructors/query_ops.rs:247:            error_message,
src/protocol/message/constructors/query_ops.rs:259:        error_message: String,
src/protocol/message/constructors/query_ops.rs:268:            error_message,
src/protocol/message/constructors/general.rs:401:        error_message: Option<String>,
src/protocol/message/constructors/general.rs:413:            error_message,
src/protocol/message/constructors/general.rs:422:        error_message: Option<String>,
src/protocol/message/constructors/general.rs:429:            error_message,
src/app_actions/handle_action/scriptlets.rs:75:    fn target_error_message(self, error: ScriptletSourceTargetError, action_id: &str) -> String {
src/app_actions/handle_action/scriptlets.rs:202:                            source_action.target_error_message(error, action_id),
tests/smoke/test-error-display.ts:13: * 4. Toast includes "Copy Error" button
src/protocol/message/variants/query_ops.rs:364:        error_message: String,
tests/agentic_await_response_preempt_parse_failure_contract.rs:126:fn await_response_truncates_error_message_to_bounded_length() {
src/main_sections/day_page_view.rs:1534:                        day_page_activation_error_message(&error.reason)
src/main_sections/day_page_view.rs:1856:fn day_page_activation_error_message(reason: &ActivationErrorReason) -> String {
src/protocol/message/variants/system_control.rs:99:        error_message: Option<String>,
src/protocol/message/variants/system_control.rs:115:        error_message: Option<String>,
src/app_actions/handle_action/scripts.rs:122:    fn target_error_message(
src/app_actions/handle_action/scripts.rs:396:                        removal_action.target_error_message(
src/app_actions/handle_action/scripts.rs:407:                        removal_action.target_error_message(
src/app_actions/handle_action/scripts.rs:419:                        removal_action.target_error_message(
src/app_actions/handle_action/shortcuts.rs:61:    fn target_error_message(self, error: ShortcutAliasTargetError, action_id: &str) -> String {
src/app_actions/handle_action/shortcuts.rs:93:    fn target_error_message(self, error: ShortcutAliasTargetError, action_id: &str) -> String {
src/app_actions/handle_action/shortcuts.rs:127:    fn target_error_message(self, error: ShortcutAliasTargetError, action_id: &str) -> String {
src/app_actions/handle_action/shortcuts.rs:177:                                shortcut_action.target_error_message(
src/app_actions/handle_action/shortcuts.rs:186:                                shortcut_action.target_error_message(
src/app_actions/handle_action/shortcuts.rs:199:                            shortcut_action.target_error_message(
src/app_actions/handle_action/shortcuts.rs:218:                            .target_error_message(ShortcutAliasTargetError::NoSelection, action_id),
src/app_actions/handle_action/shortcuts.rs:285:                            remove_action.target_error_message(
src/app_actions/handle_action/shortcuts.rs:296:                            .target_error_message(ShortcutAliasTargetError::NoSelection, action_id),
src/app_actions/handle_action/shortcuts.rs:319:                                alias_action.target_error_message(
src/app_actions/handle_action/shortcuts.rs:332:                            alias_action.target_error_message(
src/app_actions/handle_action/shortcuts.rs:351:                            .target_error_message(ShortcutAliasTargetError::NoSelection, action_id),
src/app_actions/handle_action/shortcuts.rs:401:                            remove_action.target_error_message(
src/app_actions/handle_action/shortcuts.rs:412:                            .target_error_message(ShortcutAliasTargetError::NoSelection, action_id),
src/main_sections/prompt_messages.rs:170:        error_message: String,
src/protocol/io/tests/parsing.rs:181:            error_message,
src/protocol/io/tests/parsing.rs:187:            let message = error_message.expect("serde diagnostic");
src/app_actions/handle_action/apps.rs:104:    fn target_error_message(self, message: Option<gpui::SharedString>) -> String {
src/app_actions/handle_action/apps.rs:279:                            open_action.target_error_message(msg),
src/prompt_handler/mod.rs:2471:                error_message,
src/prompt_handler/mod.rs:2480:                    error_message = %error_message,
src/prompt_handler/mod.rs:2505:                let hud_message = if error_message.chars().count() > 140 {
src/prompt_handler/mod.rs:2507:                    let truncated: String = error_message.chars().take(137).collect();
src/prompt_handler/mod.rs:2510:                    format!("Script Error: {}", error_message)
src/prompt_handler/mod.rs:2517:                let toast = Toast::error(error_message.clone(), &self.theme)
src/prompt_handler/mod.rs:2525:                        "Copy Error",
src/prompt_handler/mod.rs:2559:                    &error_message,
src/prompt_handler/mod.rs:2571:                    exit_code, error_message
src/prompt_handler/mod.rs:8285:                            chat.set_message_error(&message_id, error.clone(), cx);
src/prompt_handler/script_error_context.rs:32:    error_message: &str,
src/prompt_handler/script_error_context.rs:42:        "The script `{script_name}` just failed when I ran it. Use the attached script snapshot and error report as context, diagnose the root cause, fix it, and verify the fix by rerunning the script or giving the exact verification result.\n\nError summary: {error_message}"
src/prompt_handler/script_error_context.rs:61:    error_message: &str,
src/prompt_handler/script_error_context.rs:68:        "# Script Failure Report\n\n## Script Path\n`{script_path}`\n\n## Error Summary\n{error_message}\n"
src/prompt_handler/script_error_context.rs:106:    error_message: &str,
src/prompt_handler/script_error_context.rs:151:        error_message,
src/prompt_handler/script_error_context.rs:205:        error_message: &str,
src/prompt_handler/script_error_context.rs:218:            error_message,
src/prompt_handler/script_error_context.rs:247:            error_message,
src/logging/mod.rs:1460:        error_message = error,
tests/handle_action.rs:87:        .error_message("slow_action")
tests/handle_action.rs:106:        .error_message("post_exit")
tests/handle_action.rs:120:    assert!(result.error_message("orphan").is_some());
tests/handle_action.rs:135:    assert!(result.error_message("noop").is_none());
tests/handle_action.rs:195:fn sdk_action_result_sent_has_no_error_message() {
tests/handle_action.rs:196:    assert!(SdkActionResult::Sent.error_message("x").is_none());
tests/handle_action.rs:200:fn sdk_action_result_error_messages_include_action_name() {
tests/handle_action.rs:202:        .error_message("delete_item")
tests/handle_action.rs:207:        .error_message("save_draft")
tests/handle_action.rs:242:fn sdk_action_result_no_effect_has_no_error_message() {
tests/handle_action.rs:244:        SdkActionResult::NoEffect.error_message("any").is_none(),
tests/handle_action.rs:398:fn cancelled_has_stable_error_code_but_no_error_message() {
tests/handle_action.rs:407:        result.error_message("delete_item").is_none(),
tests/handle_action.rs:446:fn error_messages_never_contain_raw_transport_enum_names() {
tests/handle_action.rs:460:        if let Some(msg) = variant.error_message("test_action") {
tests/handle_action.rs:464:                    "error_message leaked variant name '{name}': {msg}"
src/render_builtins/flow_ux.rs:753:                        chat.set_message_error(
src/render_builtins/flow_ux.rs:885:        // happen off the GPUI thread; failures surface as SessionFailed
src/render_builtins/flow_ux.rs:1228:                chat.set_message_error(&message_id, note, cx);
src/render_builtins/flow_ux.rs:1332:                        FlowTurnOutcome::Failed(error.unwrap_or_else(|| "Turn failed".to_string())),
src/render_builtins/flow_ux.rs:1337:            FlowThreadEvent::SessionFailed { session_id, error } => {
src/render_builtins/flow_ux.rs:1425:                            run.error_message
src/protocol/types/grid_layout.rs:141:    pub error_message: String,
src/protocol/types/grid_layout.rs:163:    pub fn new(error_message: String, script_path: String) -> Self {
src/protocol/types/grid_layout.rs:165:            error_message,
src/flows/runner.rs:134:        registry.mark_failed(local_id, "mdflow CLI not found on PATH (npm i -g mdflow)");
src/flows/runner.rs:140:            registry.mark_failed(local_id, "Threadline run missing its turn prompt");
src/flows/runner.rs:146:                registry.mark_failed(local_id, &err);
src/flows/runner.rs:162:            registry.mark_failed(local_id, &format!("failed to spawn {binary}: {err}"));
src/flows/runner.rs:184:            registry.mark_failed(local_id, "child stdout unavailable");
src/flows/runner.rs:249:            registry.mark_failed(local_id, &format!("protocol violation: {detail}"));
src/flows/runner.rs:280:                    Ok(status) if status.success() => registry.mark_failed(
src/flows/runner.rs:284:                    Ok(status) => registry.mark_failed(
src/flows/runner.rs:289:                        registry.mark_failed(local_id, &format!("wait failed: {err}{stderr_note}"))
src/menu_syntax/templates.rs:482:    fn template_payload_path_error_message_is_actionable() {
src/flows/automation.rs:70:                "errorMessage": run.error_message,
src/flows/codex_client.rs:67:    SessionFailed { session_id: u64, error: String },
src/flows/codex_client.rs:138:            shared.events.push(FlowThreadEvent::SessionFailed {
src/flows/codex_client.rs:183:            shared.events.push(FlowThreadEvent::SessionFailed {
src/flows/codex_client.rs:426:        shared.events.push(FlowThreadEvent::SessionFailed {
src/flows/codex_client.rs:592:                shared.events.push(FlowThreadEvent::SessionFailed {
src/flows/codex_client.rs:603:            shared.events.push(FlowThreadEvent::SessionFailed {
src/flows/codex_client.rs:652:                shared.events.push(FlowThreadEvent::SessionFailed {
src/flows/run_registry.rs:50:    pub error_message: Option<String>,
src/flows/run_registry.rs:84:            RunPhase::Failed => match (self.exit_code, &self.error_message) {
src/flows/run_registry.rs:182:            error_message: None,
src/flows/run_registry.rs:352:                    run.error_message = Some(message.clone());
src/flows/run_registry.rs:370:    pub fn mark_failed(&self, local_id: u64, message: &str) {
src/flows/run_registry.rs:375:                run.error_message = Some(message.to_string());
src/flows/run_registry.rs:689:        assert_eq!(run2.error_message.as_deref(), Some("boom"));
src/flows/run_registry.rs:724:        registry.mark_failed(id, "reader thread exit");
src/flows/run_registry.rs:1020:        registry.mark_failed(bare, "boom");
src/flows/run_registry.rs:1021:        registry.mark_failed(convo, "boom");
src/agents/executor.rs:66:    let error_message = if !mdflow_available {
src/agents/executor.rs:83:        error_message,
src/agents/executor.rs:523:        if let Some(ref error) = availability.error_message {
src/agents/executor.rs:599:        assert!(availability.mdflow_available || availability.error_message.is_some());
src/agents/executor.rs:607:            error_message: None,
src/agents/executor.rs:614:            error_message: Some("mdflow not found".to_string()),
src/agents/executor.rs:624:            error_message: Some("mdflow not found".to_string()),
src/agents/executor.rs:637:            error_message: Some("claude CLI not found".to_string()),
src/agents/types.rs:354:    pub error_message: Option<String>,
src/agents/types.rs:525:            error_message: None,
src/agents/types.rs:535:            error_message: Some("mdflow not found".to_string()),
src/agents/types.rs:545:            error_message: Some("claude not found".to_string()),
src/execute_script/mod.rs:2652:                                                let error_message =
src/execute_script/mod.rs:2653:                                                    executor::extract_error_message(stderr_text);
src/execute_script/mod.rs:2664:                                                        error_message,
src/execute_script/mod.rs:2675:                                                        error_message: format!(
src/execute_script/mod.rs:2713:                                        let error_message =
src/execute_script/mod.rs:2714:                                            executor::extract_error_message(stderr_text);
src/execute_script/mod.rs:2720:                                            error_message,
src/scriptlet_tests/chunk_06.rs:41:        .error_message
src/scriptlet_tests/chunk_06.rs:153:    assert!(!error.error_message.is_empty());
src/scriptlet_tests/chunk_06.rs:154:    assert!(error.error_message.contains("code block"));
src/scriptlet_tests/chunk_06.rs:170:    assert!(result.errors[0].error_message.contains("Empty"));
src/scriptlets/mod.rs:365:    pub error_message: String,
src/scriptlets/mod.rs:374:        error_message: impl Into<String>,
src/scriptlets/mod.rs:380:            error_message: error_message.into(),
src/scriptlets/mod.rs:393:        write!(f, ": {}", self.error_message)
src/scriptlets/tests/chunk_06.rs:41:        .error_message
src/scriptlets/tests/chunk_06.rs:153:    assert!(!error.error_message.is_empty());
src/scriptlets/tests/chunk_06.rs:154:    assert!(error.error_message.contains("code block"));
src/scriptlets/tests/chunk_06.rs:170:    assert!(result.errors[0].error_message.contains("Empty"));
src/platform/path_actions.rs:5:        let error_message = format!(
src/platform/path_actions.rs:18:        error_message
src/platform/path_actions.rs:33:        let error_message = format!(
src/platform/path_actions.rs:43:        return Err(error_message);
src/platform/path_actions.rs:54:            let error_message = format!(
src/platform/path_actions.rs:64:            return Err(error_message);
src/platform/path_actions.rs:69:            let error_message = format!(
src/platform/path_actions.rs:79:            return Err(error_message);
src/platform/path_actions.rs:86:            let error_message = format!(
src/platform/path_actions.rs:96:            return Err(error_message);
src/platform/path_actions.rs:103:            let error_message = format!(
src/platform/path_actions.rs:113:            return Err(error_message);
src/platform/path_actions.rs:153:        let error_message = format!(
src/platform/path_actions.rs:167:        error_message
src/platform/path_actions.rs:192:        let error_message = format!(
src/platform/path_actions.rs:202:        return Err(error_message);
src/platform/path_actions.rs:213:            let error_message = format!(
src/platform/path_actions.rs:223:            return Err(error_message);
src/platform/path_actions.rs:228:            let error_message = format!(
src/platform/path_actions.rs:238:            return Err(error_message);
src/platform/path_actions.rs:245:            let error_message = format!(
src/platform/path_actions.rs:255:            return Err(error_message);
src/platform/path_actions.rs:269:            let error_message = format!(
src/platform/path_actions.rs:279:            return Err(error_message);
src/platform/path_actions.rs:302:        let error_message = format!(
src/platform/path_actions.rs:314:        error_message
src/platform/path_actions.rs:337:        let error_message =
src/platform/path_actions.rs:345:        return Err(error_message);
src/platform/path_actions.rs:354:            let error_message =
src/platform/path_actions.rs:361:            return Err(error_message);
src/platform/path_actions.rs:368:            let error_message =
src/platform/path_actions.rs:375:            return Err(error_message);
src/platform/path_actions.rs:382:            let error_message =
src/platform/path_actions.rs:389:            return Err(error_message);
src/platform/path_actions.rs:396:            let error_message =
src/platform/path_actions.rs:403:            return Err(error_message);
src/platform/path_actions.rs:426:        let error_message = format!(
src/platform/path_actions.rs:436:        error_message
src/platform/path_actions.rs:440:        let error_message = format!(
src/platform/path_actions.rs:450:        error_message
src/shortcuts/types_tests.rs:33:fn parse_error_messages_include_recovery_guidance() {
src/ai/providers.rs:27:fn extract_api_error_message(body: &str) -> Option<String> {
src/ai/providers.rs:77:    let error_detail = extract_api_error_message(&body_str);
src/ai/providers.rs:425:    fn stream_message(
src/ai/providers.rs:622:    fn stream_message(
src/ai/providers.rs:887:    fn stream_message(
src/ai/providers.rs:1109:    fn stream_message(
src/ai/providers.rs:1295:    fn stream_message(
src/ai/providers.rs:1618:    fn stream_message(
src/ai/providers.rs:2230:    fn stream_message(
src/ai/providers.rs:2539:            .stream_message(
src/ai/providers.rs:2822:    fn test_extract_api_error_message_openai_format() {
src/ai/providers.rs:2825:        let result = extract_api_error_message(body);
src/ai/providers.rs:2833:        let result = extract_api_error_message(body);
src/ai/providers.rs:2838:    fn test_extract_api_error_message_anthropic_format() {
src/ai/providers.rs:2841:        let result = extract_api_error_message(body);
src/ai/providers.rs:2849:    fn test_extract_api_error_message_invalid_json() {
src/ai/providers.rs:2850:        let result = extract_api_error_message("not json");
src/ai/providers.rs:2853:        let result = extract_api_error_message(r#"{"foo": "bar"}"#);
src/ai/providers.rs:3272:        let result = provider.stream_message(
src/scriptlets/tests/chunk_09.rs:495:            .error_message
src/main_entry/runtime_stdin.rs:607:                                let (ok, text_length, error_code, error_message) = match result {
src/main_entry/runtime_stdin.rs:638:                                                error_message,
src/main_entry/runtime_stdin.rs:659:                                let (error_code, error_message) = match result {
src/main_entry/runtime_stdin.rs:691:                                                error_message,
src/main_entry/runtime_stdin_match_tail.rs:158:                                let (ok, text_length, error_code, error_message) = match result {
src/main_entry/runtime_stdin_match_tail.rs:189:                                                error_message,
src/main_entry/runtime_stdin_match_tail.rs:210:                                let (error_code, error_message) = match result {
src/main_entry/runtime_stdin_match_tail.rs:242:                                                error_message,
src/components/toast/model.rs:21:/// - Action buttons (e.g., "Copy Error", "View Details")
src/main_entry/app_run_setup.rs:2684:                                let (ok, outcome_code, entry_id, error_code, error_message) =
src/main_entry/app_run_setup.rs:2732:                                            error_message,
src/main_entry/app_run_setup.rs:2895:                                let (ok, text_length, error_code, error_message) = match result {
src/main_entry/app_run_setup.rs:2926:                                                error_message,
src/main_entry/app_run_setup.rs:2947:                                let (error_code, error_message) = match result {
src/main_entry/app_run_setup.rs:2979:                                                error_message,
src/app_impl/selection_fallback.rs:1459:                            let message = calculate_fallback_error_message(&expression);
src/app_impl/tests.rs:1:use super::{calculate_fallback_error_message, ScriptListApp};
src/app_impl/tests.rs:79:fn test_calculate_fallback_error_message_includes_expression_and_recovery() {
src/app_impl/tests.rs:80:    let message = calculate_fallback_error_message("2 + )");
tests/source_audits/focused_text_agent_chat_entry.rs:581:        "error_message: Option<String>",
src/ai/agent_chat/codex_exec.rs:376:                        let _ = event_tx.send_blocking(AgentChatEvent::Failed { error: message });
src/ai/agent_chat/codex_exec.rs:849:                    "quick_ai_more_than_two_search_queries",
src/ai/agent_chat/codex_exec.rs:1463:    fn codex_quick_ai_more_than_two_search_queries_fails_closed() {
src/ai/agent_chat/codex_exec.rs:1473:        assert_eq!(error.message, "quick_ai_more_than_two_search_queries");
src/ai/agent_chat/pi/events.rs:19:        return vec![AgentChatEvent::Failed {
src/ai/agent_chat/pi/events.rs:138:                vec![AgentChatEvent::Failed {
src/ai/agent_chat/pi/events.rs:147:        Some("event_serialize_error") | Some("extension_error") => vec![AgentChatEvent::Failed {
src/ai/agent_chat/pi/events.rs:744:            [AgentChatEvent::Failed { error }] if error == "nope"
src/ai/agent_chat/pi/events.rs:757:            [AgentChatEvent::Failed { error }] if error == "failed"
src/ai/agent_chat/pi/events.rs:832:            [AgentChatEvent::Failed { error }] if error == "entry not found"
src/ai/agent_chat/pi/runtime.rs:257:                                .send(AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:274:                                    .send(AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:306:                            .send(AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:461:                    .send(AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:479:                .send(AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:503:                .send(AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:520:            .send(AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:540:                        .send(AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:548:                        .send(AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:579:                    AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:608:                    AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:626:                AgentChatEvent::TurnFinished { .. } | AgentChatEvent::Failed { .. }
src/ai/agent_chat/pi/runtime.rs:646:            AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:757:                    AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:785:                    AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:801:                AgentChatEvent::TurnFinished { .. } | AgentChatEvent::Failed { .. }
src/ai/agent_chat/pi/runtime.rs:829:        AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:882:                    .send(AgentChatEvent::Failed {
src/ai/agent_chat/pi/runtime.rs:1167:                AgentChatEvent::Failed { error } if error.contains("exited before responding")
src/ai/agent_chat/pi/runtime.rs:1196:                    AgentChatEvent::Failed { error } if error.contains("exited before responding")
src/action_helpers/tests.rs:523:    assert!(result.error_message("test").is_some());
src/action_helpers/tests.rs:551:    assert!(result.error_message("test").is_none());
src/action_helpers/tests.rs:617:    assert!(result.error_message("test").is_none());
src/action_helpers/tests.rs:624:    // Cancelled has error_code (for machine consumption) but no error_message (no toast)
src/action_helpers/tests.rs:633:    assert!(cancelled.error_message("test").is_none());
src/action_helpers/tests.rs:634:    assert!(no_sender.error_message("test").is_some());
src/action_helpers/tests.rs:660:    let err = result.error_message("busy_action").unwrap();
src/action_helpers/tests.rs:686:    let err = result.error_message("late_action").unwrap();
src/action_helpers/tests.rs:826:fn sdk_action_result_error_message_never_contains_variant_names() {
src/action_helpers/tests.rs:837:        if let Some(msg) = variant.error_message("test") {
src/action_helpers/tests.rs:840:                "error_message leaked variant name 'ChannelFull': {msg}"
src/action_helpers/tests.rs:844:                "error_message leaked variant name 'ChannelDisconnected': {msg}"
src/action_helpers/tests.rs:848:                "error_message leaked variant name 'NoSender': {msg}"
src/action_helpers/tests.rs:852:                "error_message leaked variant name 'Cancelled': {msg}"
src/action_helpers/tests.rs:863:fn cancelled_variant_has_error_code_but_no_error_message() {
src/action_helpers/tests.rs:873:        result.error_message("any_action").is_none(),
src/ai/agent_chat/warm_session.rs:858:            Ok(AgentChatEvent::Failed { error }) => {
tests/source_audits/action_shortcut_alias.rs:53:            && block.contains("target_error_message(ShortcutAliasTargetError::NoSelection"),
tests/source_audits/action_shortcut_alias.rs:68:        block.contains("shortcut_action.target_error_message(")
tests/source_audits/action_shortcut_alias.rs:217:        block.contains("remove_action.target_error_message(")
tests/source_audits/action_shortcut_alias.rs:258:            && block.contains("target_error_message(ShortcutAliasTargetError::NoSelection"),
tests/source_audits/action_shortcut_alias.rs:273:        block.contains("alias_action.target_error_message(")
tests/source_audits/action_shortcut_alias.rs:343:        block.contains("remove_action.target_error_message(")
src/notes/window/notes_actions.rs:110:                    activation_error_message(&error.reason)
src/notes/window/notes_actions.rs:738:fn activation_error_message(reason: &ActivationErrorReason) -> String {
src/ai/agent_chat/ui/view.rs:2885:                    | AgentChatEvent::Failed { .. }
src/ai/agent_chat/ui/view.rs:2910:            AgentChatEvent::Failed { error } => {
src/ai/agent_chat/ui/view.rs:2950:                        | AgentChatEvent::Failed { .. }
src/ai/agent_chat/ui/view.rs:11306:        if callout.raw_detail.is_some() {
src/ai/agent_chat/ui/view.rs:11310:                    "Copy Error",
src/ai/agent_chat/ui/view.rs:11340:        let (raw_detail, auth_recovery, selected_model_id) = {
src/ai/agent_chat/ui/view.rs:11346:                callout.raw_detail.as_ref().map(ToString::to_string),
src/ai/agent_chat/ui/view.rs:11353:            let Some(raw_detail) = raw_detail else {
src/ai/agent_chat/ui/view.rs:11356:            cx.write_to_clipboard(gpui::ClipboardItem::new_string(raw_detail));
src/ai/agent_chat/ui/view.rs:11375:                raw_detail.as_deref(),
src/extension_types/mod.rs:503:    pub error_message: String,
src/extension_types/mod.rs:511:        error_message: impl Into<String>,
src/extension_types/mod.rs:517:            error_message: error_message.into(),
src/extension_types/mod.rs:530:        write!(f, ": {}", self.error_message)
src/clipboard_history/ocr.rs:366:        let error_message = if perform_error != nil {
src/clipboard_history/ocr.rs:378:            error_message
src/ai/agent_chat/ui/thread/tests.rs:1030:fn failed_event_creates_error_message_and_retryable_callout() {
src/ai/agent_chat/ui/thread/tests.rs:1036:        AgentChatEvent::Failed {
src/ai/agent_chat/ui/thread/tests.rs:1047:    assert_eq!(callout.title.as_ref(), "Turn failed");
src/ai/agent_chat/ui/thread/tests.rs:1060:        AgentChatEvent::Failed {
src/ai/agent_chat/ui/thread/tests.rs:1072:    assert_eq!(callout.raw_detail.as_ref().unwrap().as_ref(), raw_error);
src/ai/agent_chat/ui/thread/tests.rs:1081:    thread.apply_event_test(AgentChatEvent::Failed {
src/ai/agent_chat/ui/thread/tests.rs:1105:    thread.apply_event_test(AgentChatEvent::Failed {
src/ai/agent_chat/ui/thread/tests.rs:1118:    thread.apply_event_test(AgentChatEvent::Failed {
src/action_helpers.rs:557:            user_message: result.error_message(action_name),
src/action_helpers.rs:573:            user_message: result.error_message(action_name),
src/action_helpers.rs:652:    pub fn error_message(&self, action_name: &str) -> Option<String> {
src/action_helpers.rs:670:    /// Unlike `error_message`, this does not require an action name and returns
src/scriptlet_cache/mod.rs:508:pub fn format_parse_error_message(errors: &[ScriptletValidationError]) -> String {
src/scriptlet_cache/mod.rs:526:                "format_parse_error_message expected one file but found none"
src/scriptlet_cache/mod.rs:563:/// - error_message: Description of the error
src/scriptlet_cache/mod.rs:584:            error.error_message
src/scriptlet_cache/mod.rs:596:            error_message = %error.error_message,
src/scriptlet_cache/mod.rs:682:        hud_message: format_parse_error_message(errors),
src/scriptlet_cache/mod.rs:1260:        assert_eq!(super::format_parse_error_message(&errors), "");
src/scriptlet_cache/mod.rs:1271:        let msg = super::format_parse_error_message(&errors);
src/scriptlet_cache/mod.rs:1283:        let msg = super::format_parse_error_message(&errors);
src/scriptlet_cache/mod.rs:1303:        let msg = super::format_parse_error_message(&errors);
src/scriptlet_cache/mod.rs:1329:        let msg = super::format_parse_error_message(&errors);
src/ai/agent_chat/ui/thread.rs:156:    /// Original provider payload retained for diagnostics and Copy Error.
src/ai/agent_chat/ui/thread.rs:157:    pub(crate) raw_detail: Option<SharedString>,
src/ai/agent_chat/ui/thread.rs:170:            raw_detail: Some(raw_error),
src/ai/agent_chat/ui/thread.rs:181:            raw_detail: None,
src/ai/agent_chat/ui/thread.rs:223:        title: "Turn failed".to_string(),
src/ai/agent_chat/ui/thread.rs:2471:                    AgentChatEvent::TurnFinished { .. } | AgentChatEvent::Failed { .. }
src/ai/agent_chat/ui/thread.rs:2955:            AgentChatEvent::Failed { error } => {
src/ai/agent_chat/ui/thread.rs:3696:                        } else if let AgentChatEvent::Failed { error } = event {
src/ai/agent_chat/ui/thread.rs:4748:            super::AgentChatEvent::Failed { error } => {
src/app_impl/mod.rs:105:pub(crate) use startup::calculate_fallback_error_message;
src/ai/agent_chat/ui/components/transcript.rs:1047:                Self::render_error_message(msg, colors, text_view_state, style_def)
src/ai/agent_chat/ui/components/transcript.rs:1667:    fn render_error_message(
src/stdin_commands/mod.rs:1674:                error_message,
src/stdin_commands/mod.rs:1680:                let message = error_message.expect("length diagnostic");
src/executor/errors.rs:88:pub fn extract_error_message(stderr: &str) -> String {
src/executor/errors.rs:229:    use super::extract_error_message;
src/executor/errors.rs:232:    fn test_extract_error_message_does_not_split_utf8_when_fallback_line_is_multibyte() {
src/executor/errors.rs:234:        let message = extract_error_message(&stderr);
src/executor/errors.rs:242:    fn test_extract_error_message_skips_sdk_noise_and_prefers_real_error() {
src/executor/errors.rs:251:            extract_error_message(stderr),
src/executor/mod.rs:27:pub use errors::{extract_error_message, generate_suggestions, parse_stack_trace};
src/app_impl/startup.rs:3:pub(super) fn calculate_fallback_error_message(expression: &str) -> String {
src/ai/agent_chat/ui/tests.rs:332:fn error_event_creates_error_message_and_sets_status() {
src/ai/agent_chat/ui/tests.rs:335:    thread.apply_event_test(AgentChatEvent::Failed {
src/ai/agent_chat/ui/view/tests.rs:88:        title: "Turn failed".into(),
src/ai/agent_chat/ui/view/tests.rs:90:        raw_detail: Some("raw provider error".into()),
src/ai/agent_chat/ui/view/tests.rs:121:    assert_eq!(model.title.as_ref(), "Turn failed");
tests/agent_chat_warm_lifecycle_contract.rs:134:        "AgentChatEvent::Failed",
tests/shortcut_error_messages.rs:4:fn test_shortcut_parse_error_messages_describe_recovery_when_input_is_invalid() {
tests/actions.rs:790:            && content.contains("source_action.target_error_message(error, action_id)"),
tests/actions.rs:883:            && content.contains("removal_action.target_error_message(")
tests/actions.rs:1263:        content.contains("shortcut_action.target_error_message(")
tests/actions.rs:1264:            && content.contains("alias_action.target_error_message(")
tests/actions.rs:1265:            && content.contains("remove_action.target_error_message(")
tests/actions.rs:3077:            && content.contains("shortcut_action.target_error_message(")
tests/actions.rs:3081:            && content.contains("alias_action.target_error_message("),
tests/actions.rs:3168:            && content.contains("open_action.target_error_message(msg)")
tests/action_helpers.rs:476:    assert!(result.error_message("test").is_some());
tests/action_helpers.rs:504:    assert!(result.error_message("test").is_none());
tests/action_helpers.rs:570:    assert!(result.error_message("test").is_none());
tests/action_helpers.rs:593:    let err = result.error_message("busy_action").unwrap();
tests/action_helpers.rs:619:    let err = result.error_message("late_action").unwrap();
tests/action_helpers.rs:644:fn cancelled_variant_produces_code_but_no_error_message() {
tests/action_helpers.rs:649:        result.error_message("delete_all").is_none(),
tests/action_helpers.rs:677:fn error_messages_never_expose_transport_enum_names() {
tests/action_helpers.rs:691:        if let Some(msg) = variant.error_message("test") {
tests/action_helpers.rs:695:                    "error_message leaked transport name '{name}': {msg}"
tests/automation_screenshots/mod.rs:236:fn ambiguous_error_message_contains_candidates_and_score() {
tests/builtin_execution.rs:156:        .error_message("run_build")
tests/builtin_execution.rs:172:        .error_message("late_action")
tests/builtin_execution.rs:181:        .error_message("orphan")
tests/builtin_execution.rs:187:fn sent_and_no_effect_produce_no_error_message() {
tests/builtin_execution.rs:188:    assert!(SdkActionResult::Sent.error_message("x").is_none());
tests/builtin_execution.rs:189:    assert!(SdkActionResult::NoEffect.error_message("x").is_none());
tests/builtin_execution.rs:235:    assert!(result.error_message("force_quit").is_none());
scripts/agentic/agent-chat-auth-recovery-probe.ts:121:    failures.push({ name: "agent_chat_error_message_count", agentState });
src/prompts/chat/render_turns.rs:84:            let error_type = ChatErrorType::from_error_string(error_str);
src/prompts/chat/render_turns.rs:85:            let error_message = error_type.display_message();
src/prompts/chat/render_turns.rs:99:                        .child(error_message.to_string()),
src/prompts/chat/render_turns.rs:130:            if !detail.is_empty() && detail != error_message {
src/prompts/chat/render_turns.rs:132:                let is_unknown = error_type == ChatErrorType::Unknown;
src/prompts/chat/streaming.rs:330:            let result = ai_provider.stream_message(
src/prompts/chat/state.rs:655:    pub fn set_message_error(&mut self, message_id: &str, error: String, cx: &mut Context<Self>) {
src/prompts/chat/mod.rs:74:    ChatErrorType, ChatEscapeCallback, ChatModel, ChatPromptHostMode, ChatRetryCallback,
src/prompts/chat/types.rs:616:pub enum ChatErrorType {
src/prompts/chat/types.rs:630:impl ChatErrorType {
src/prompts/chat/types.rs:637:            ChatErrorType::NoApiKey
src/prompts/chat/types.rs:641:            ChatErrorType::ClaudeCodeNested
src/prompts/chat/types.rs:647:            ChatErrorType::ClaudeCodeNotFound
src/prompts/chat/types.rs:652:            ChatErrorType::NetworkError
src/prompts/chat/types.rs:654:            ChatErrorType::StreamInterrupted
src/prompts/chat/types.rs:656:            ChatErrorType::RateLimited
src/prompts/chat/types.rs:664:            ChatErrorType::InvalidModel
src/prompts/chat/types.rs:669:            ChatErrorType::TokenLimit
src/prompts/chat/types.rs:676:            ChatErrorType::ServerError
src/prompts/chat/types.rs:678:            ChatErrorType::ProviderError
src/prompts/chat/types.rs:680:            ChatErrorType::Unknown
src/prompts/chat/types.rs:686:            ChatErrorType::NoApiKey => {
src/prompts/chat/types.rs:689:            ChatErrorType::NetworkError => {
src/prompts/chat/types.rs:692:            ChatErrorType::StreamInterrupted => {
src/prompts/chat/types.rs:695:            ChatErrorType::RateLimited => {
src/prompts/chat/types.rs:698:            ChatErrorType::InvalidModel => "\u{26a0} Model unavailable. Using default model.",
src/prompts/chat/types.rs:699:            ChatErrorType::TokenLimit => "\u{26a0} Message too long. Try a shorter prompt.",
src/prompts/chat/types.rs:700:            ChatErrorType::ClaudeCodeNested => {
src/prompts/chat/types.rs:704:            ChatErrorType::ClaudeCodeNotFound => {
src/prompts/chat/types.rs:708:            ChatErrorType::ProviderError => "\u{26a0} AI provider error. Check the details below.",
src/prompts/chat/types.rs:709:            ChatErrorType::ServerError => {
src/prompts/chat/types.rs:712:            ChatErrorType::Unknown => "\u{26a0} Something went wrong. Please try again.",
src/prompts/chat/types.rs:719:            ChatErrorType::NetworkError
src/prompts/chat/types.rs:720:                | ChatErrorType::StreamInterrupted
src/prompts/chat/types.rs:721:                | ChatErrorType::RateLimited
src/prompts/chat/types.rs:722:                | ChatErrorType::ProviderError
src/prompts/chat/types.rs:723:                | ChatErrorType::ServerError
src/prompts/chat/types.rs:724:                | ChatErrorType::Unknown
```

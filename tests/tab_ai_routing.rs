//! Contract tests verifying Agent Chat compatibility routes and Agent Chat entry points.
//!
//! The primary Agent Chat entry key is Command+Enter. Plain Tab is deprecated
//! for Agent Chat entry and remains local to focus/navigation handlers.
//!
//! Direct zero-intent launcher surfaces now route through
//! `open_tab_ai_agent_chat_with_entry_intent(None, cx)`, while `open_tab_ai_chat()`
//! remains a compatibility wrapper for older harness entry callers.

const TAB_SOURCE: &str = include_str!("../src/app_impl/startup.rs");
const TAB_NEW_SOURCE: &str = include_str!("../src/app_impl/startup_new_tab.rs");
const TAB_AI_MODE_SOURCE: &str = include_str!("../src/app_impl/agent_handoff/mod.rs");
const TAB_AI_AGENT_CHAT_LAUNCH_SOURCE: &str =
    include_str!("../src/app_impl/agent_handoff/agent_chat_launch.rs");
const RENDER_IMPL_SOURCE: &str = include_str!("../src/main_sections/render_impl.rs");
const SCRIPT_LIST_SOURCE: &str = include_str!("../src/render_script_list/mod.rs");
const APP_STATE_SOURCE: &str = include_str!("../src/main_sections/app_state.rs");
const APP_VIEW_STATE_SOURCE: &str = include_str!("../src/main_sections/app_view_state.rs");
const HARNESS_SOURCE: &str = include_str!("../src/ai/harness/mod.rs");
const TERM_RENDER_SOURCE: &str = include_str!("../src/render_prompts/term.rs");
const AI_MOD_SOURCE: &str = include_str!("../src/ai/mod.rs");
const BUILTIN_EXECUTION_SOURCE: &str = include_str!("../src/app_execute/builtin_execution.rs");
const PROMPT_AI_SOURCE: &str = include_str!("../src/app_impl/prompt_ai.rs");
const BUILTINS_SOURCE: &str = include_str!("../src/builtins/mod.rs");
const SCREENSHOT_FILES_SOURCE: &str = include_str!("../src/ai/harness/screenshot_files.rs");
const ACTIONS_DIALOG_SOURCE: &str = include_str!("../src/app_impl/actions_dialog.rs");
const ACTIONS_WINDOW_SOURCE: &str = include_str!("../src/actions/window.rs");
const AI_MOD_SOURCE_FOR_ACTIONS: &str = include_str!("../src/ai/mod.rs");

fn source_block_after<'a>(source: &'a str, needle: &str, len: usize) -> &'a str {
    let start = source
        .find(needle)
        .unwrap_or_else(|| panic!("expected to find `{needle}`"));
    &source[start..source.len().min(start + len)]
}

// =========================================================================
// Primary contract: Tab → harness terminal (QuickTerminalView)
// =========================================================================

#[test]
fn plain_tab_agent_chat_entry_is_removed_and_global_cmd_enter_bails_when_portal_open() {
    // Regression: when an attachment portal (e.g. `@` mention → file /
    // script / scriptlet / skill search) is open, the visible surface
    // reuses `AppView::ScriptList`. Plain Tab is no longer an Agent Chat
    // entry at all, and global Cmd+Enter must still refuse portal submits.

    assert!(
        !TAB_AI_MODE_SOURCE.contains("try_route_plain_tab_to_agent_chat_context_capture")
            && !TAB_AI_MODE_SOURCE.contains("agent_chat_plain_tab_entry_deprecated")
            && !TAB_AI_MODE_SOURCE.contains("tab_ai_plain_tab_routed_to_agent_chat"),
        "Plain Tab must not keep an Agent Chat route, shim, or telemetry marker"
    );

    let cmd_enter_fn = TAB_AI_MODE_SOURCE
        .find("fn try_route_global_cmd_enter_to_agent_chat_context_capture")
        .expect("global cmd+enter helper must exist");
    let cmd_enter_body = &TAB_AI_MODE_SOURCE
        [cmd_enter_fn..cmd_enter_fn + 1500.min(TAB_AI_MODE_SOURCE.len() - cmd_enter_fn)];
    assert!(
        cmd_enter_body.contains("self.is_in_attachment_portal()")
            && cmd_enter_body.contains("return false"),
        "Global Cmd+Enter Agent Chat helper must early-return false when an attachment portal is active"
    );
}

#[test]
fn global_cmd_enter_spine_probe_falls_through_to_agent_chat_route() {
    let cmd_enter_body = source_block_after(
        TAB_AI_MODE_SOURCE,
        "fn try_route_global_cmd_enter_to_agent_chat_context_capture",
        2600,
    );

    assert!(
        cmd_enter_body.contains("script_list_cmd_enter_has_explicit_spine_prompt_plan")
            && cmd_enter_body.contains("if self.try_submit_spine_prompt_plan_from_enter(cx)")
            && cmd_enter_body.contains("return true;"),
        "Global Cmd+Enter may let Spine consume explicit prompt-builder syntax only after \
         an explicit predicate, and a false Spine probe must fall through to the normal \
         Agent Chat route"
    );
    assert!(
        !cmd_enter_body.contains("return self.try_submit_spine_prompt_plan_from_enter(cx);"),
        "Global Cmd+Enter must not return the Spine probe result directly; plain launcher \
         prose should continue to the Agent Chat route"
    );
    assert!(
        cmd_enter_body.contains("tab_ai_global_cmd_enter_routed_to_agent_chat"),
        "Global Cmd+Enter fallthrough must still reach the Agent Chat route telemetry"
    );
}

#[test]
fn quick_terminal_output_routes_through_text_block_context_part() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("quick_terminal_context_part_from_entity")
            && TAB_AI_MODE_SOURCE.contains("AiContextPart::TextBlock")
            && TAB_AI_MODE_SOURCE.contains("text/x-terminal-transcript")
            && TAB_AI_MODE_SOURCE
                .contains("open_tab_ai_agent_chat_with_context_part(part, \"quick_terminal_output\""),
        "Quick Terminal output must attach to Agent Chat as a TextBlock via the existing context-part handoff"
    );
}

#[test]
fn terminal_context_selector_portal_opens_quick_terminal() {
    let picker_source = include_str!("../src/ai/context_selector/mod.rs");
    let attachment_portal_source = include_str!("../src/app_impl/attachment_portal.rs");
    assert!(
        picker_source.contains("ContextPortalKind::Terminal")
            && picker_source.contains("label: \"@terminal\"")
            && picker_source.contains("match_terms: &[\"terminal\""),
        "Terminal must be offered by the shared context menu as a portal option"
    );
    assert!(
        attachment_portal_source.contains("ContextPortalKind::Terminal =>")
            && attachment_portal_source.contains("self.open_quick_terminal(None, cx);"),
        "Terminal portal selection must open the Quick Terminal workflow"
    );
}

#[test]
fn quick_terminal_cmd_enter_does_not_add_new_agent_chat_surface_mutation_path() {
    let handoff = source_block_after(
        TAB_AI_MODE_SOURCE,
        "pub(crate) fn open_agent_chat_with_quick_terminal_output",
        2200,
    );
    for forbidden in [
        "self.current_view = AppView::AgentChatView",
        "ensure_embedded_ai_window(",
        "transition_agent_chat_surface(",
        "thread.add_context_part",
    ] {
        assert!(
            !handoff.contains(forbidden),
            "Quick Terminal handoff must not bypass Agent Chat entry/context helpers: {forbidden}"
        );
    }
}

#[test]
fn pi_warmup_branch_opens_visible_chat_instead_of_warming_setup() {
    let launch_body = source_block_after(
        TAB_AI_AGENT_CHAT_LAUNCH_SOURCE,
        "fn open_tab_ai_pi_view_from_launch(",
        6500,
    );

    assert!(
        launch_body.contains("acquire_ready_or_spawn_cold(warm_spec)"),
        "Pi Agent Chat warm miss must acquire a ready lease or cold-spawn an acquired lease"
    );
    assert!(
        launch_body.contains("enter_embedded_agent_chat_surface"),
        "Pi Agent Chat open must still switch into the visible Agent Chat view"
    );
    assert!(
        !launch_body.contains("show_pi_agent_chat_warming_setup_view"),
        "Ordinary Empty/Preparing warm misses must not show the retry-only warming setup"
    );
    assert!(
        !launch_body.contains("Starting Pi Agent Chat. Try again in a moment."),
        "Ordinary Empty/Preparing warm misses must not toast a retry-only waiting state"
    );
}

#[test]
fn open_tab_ai_chat_routes_to_harness_terminal() {
    // open_tab_ai_chat must unconditionally call open_tab_ai_harness_terminal.
    let open_fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_tab_ai_chat(")
        .expect("open_tab_ai_chat must exist");
    let open_fn_body = &TAB_AI_MODE_SOURCE[open_fn_start..];
    let next_fn = open_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(open_fn_body.len());
    let open_fn_body = &open_fn_body[..next_fn];

    assert!(
        open_fn_body.contains("open_tab_ai_chat_with_entry_intent(None, cx)")
            || open_fn_body.contains("begin_tab_ai_harness_entry("),
        "open_tab_ai_chat must delegate to the harness terminal as the primary surface"
    );
    assert!(
        !open_fn_body.contains("open_tab_ai_full_view_chat"),
        "open_tab_ai_chat must NOT route to the legacy inline chat"
    );
}

#[test]
fn open_tab_ai_chat_does_not_create_tab_ai_chat_entity() {
    // The primary entry must not instantiate TabAiChat — it uses TermPrompt instead.
    let open_fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_tab_ai_chat(")
        .expect("open_tab_ai_chat must exist");
    let open_fn_body = &TAB_AI_MODE_SOURCE[open_fn_start..];
    let next_fn = open_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(open_fn_body.len());
    let open_fn_body = &open_fn_body[..next_fn];

    assert!(
        !open_fn_body.contains("TabAiChat::new"),
        "open_tab_ai_chat must not create a TabAiChat entity — the harness terminal is primary"
    );
}

#[test]
fn startup_routes_cmd_enter_into_harness_terminal() {
    assert!(
        TAB_SOURCE.contains("try_route_global_cmd_enter_to_agent_chat_context_capture"),
        "startup.rs must route Cmd+Enter through the Agent Chat context-capture entry point"
    );
    assert!(
        !TAB_SOURCE.contains("open_tab_ai_overlay"),
        "startup.rs must not reference the removed overlay opener"
    );
    assert!(
        !TAB_SOURCE.contains("open_tab_ai_full_view_chat"),
        "startup.rs must not call the legacy inline chat directly"
    );
}

#[test]
fn tab_ai_creates_quick_terminal_view() {
    // The harness terminal function must switch to QuickTerminalView.
    let open_fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_tab_ai_harness_terminal_from_request(")
        .expect("open_tab_ai_harness_terminal_from_request must exist");
    let open_fn_body = &TAB_AI_MODE_SOURCE[open_fn_start..];
    let next_fn = open_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(open_fn_body.len());
    let open_fn_body = &open_fn_body[..next_fn];

    assert!(
        open_fn_body.contains("AppView::QuickTerminalView"),
        "open_tab_ai_harness_terminal_from_request must switch to QuickTerminalView"
    );
}

#[test]
fn tab_ai_reads_harness_config() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("read_tab_ai_harness_config"),
        "harness terminal path must load harness configuration before spawning"
    );
}

#[test]
fn tab_ai_uses_persistent_harness_session_state() {
    assert!(
        APP_STATE_SOURCE.contains("tab_ai_harness: Option<crate::ai::TabAiHarnessSessionState>"),
        "app_state.rs must persist the Tab AI harness session"
    );
}

// =========================================================================
// Tab interceptor routing order
// =========================================================================

#[test]
fn cmd_enter_routing_file_search_uses_shared_global_route() {
    // FileSearch plain Cmd+Enter now routes through the same shared global
    // Agent Chat context-capture path as other launcher surfaces. The global route
    // fires in the startup interceptor, and FileSearch is eligible because
    // supports_global_cmd_enter_ai_entry includes FileSearchView.
    assert!(
        TAB_SOURCE.contains("try_route_global_cmd_enter_to_agent_chat_context_capture"),
        "global Cmd+Enter Agent Chat route must exist in startup interceptor"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("AppView::FileSearchView { .. }"),
        "FileSearchView must be eligible for the shared global Cmd+Enter route"
    );
}

#[test]
fn tab_ai_routing_preserves_chat_prompt_tab_setup_only() {
    assert!(
        TAB_SOURCE.contains("if matches!(this.current_view, AppView::ChatPrompt { .. }) {")
            && TAB_SOURCE.contains("chat.handle_setup_key(\"tab\", true, cx)")
            && !TAB_SOURCE.contains("tab_ai_chat_prompt_plain_tab_to_agent_chat"),
        "ChatPrompt Tab handling must keep only its local setup/back-tab path",
    );
}

#[test]
fn tab_ai_routing_removes_script_list_shift_tab_quick_submit() {
    assert!(
        !TAB_SOURCE.contains("submit_to_current_or_new_tab_ai_harness_from_text"),
        "Shift+Tab must no longer route typed ScriptList queries through the quick-submit planner"
    );
}

// =========================================================================
// Entry-intent normalization and submit mode selection
// =========================================================================

#[test]
fn entry_intent_is_trimmed_before_submit_mode_is_selected() {
    assert!(
        TAB_AI_MODE_SOURCE.contains(".map(|value| value.trim().to_string())"),
        "entry intent must be trimmed before mode selection",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains(".filter(|value| !value.is_empty())"),
        "whitespace-only entry intent must collapse to None",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("let submit_now = request"),
        "submission mode must derive from quick_submit_plan or entry_intent",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("let submission_mode = if submit_now"),
        "submission mode must branch on the resolved submit_now flag",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("crate::ai::TabAiHarnessSubmissionMode::Submit"),
        "non-empty normalized entry intent must use Submit mode",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("crate::ai::TabAiHarnessSubmissionMode::PasteOnly"),
        "empty normalized entry intent must use PasteOnly mode",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("effective_intent"),
        "quick-submit plan's synthesized_intent must be preferred over raw entry_intent",
    );
}

// =========================================================================
// Tab re-entry guards
// =========================================================================

#[test]
fn startup_tab_interceptor_does_not_steal_tab_from_quick_terminal() {
    assert!(
        TAB_SOURCE.contains("AppView::QuickTerminalView"),
        "startup.rs must special-case QuickTerminalView"
    );
    // The QuickTerminalView Tab handler writes raw bytes directly to the PTY
    // and stops propagation so GPUI focus traversal does not consume the Tab.
    assert!(
        TAB_SOURCE.contains("term.terminal.input(bytes)"),
        "QuickTerminalView Tab handling must write raw bytes to the PTY"
    );
    assert!(
        TAB_SOURCE.contains("b\"\\t\""),
        "plain Tab must be written as a tab byte"
    );
    assert!(
        TAB_SOURCE.contains("b\"\\x1b[Z\""),
        "Shift+Tab must be written as a backtab escape sequence"
    );
    assert!(
        TAB_SOURCE.contains("cx.stop_propagation();"),
        "QuickTerminalView Tab handling must stop propagation so GPUI focus traversal does not consume the key"
    );
}

#[test]
fn startup_new_tab_interceptor_writes_tab_bytes_directly_to_quick_terminal_pty() {
    assert!(
        TAB_NEW_SOURCE.contains("AppView::QuickTerminalView"),
        "startup_new_tab.rs must special-case QuickTerminalView"
    );
    assert!(
        TAB_NEW_SOURCE.contains("term.terminal.input(bytes)"),
        "QuickTerminalView Tab handling must write raw bytes to the PTY"
    );
    assert!(
        TAB_NEW_SOURCE.contains("b\"\\t\""),
        "plain Tab must be written as a tab byte"
    );
    assert!(
        TAB_NEW_SOURCE.contains("b\"\\x1b[Z\""),
        "Shift+Tab must be written as a backtab escape sequence"
    );
    assert!(
        TAB_NEW_SOURCE.contains("cx.stop_propagation();"),
        "QuickTerminalView Tab handling must stop propagation so GPUI focus traversal does not consume the key"
    );
}

#[test]
fn startup_tab_interceptor_blocks_reentry_when_save_offer_is_open() {
    assert!(
        TAB_SOURCE.contains("tab_ai_save_offer_state.is_some()"),
        "startup_new_tab.rs must block Tab reentry while the save-offer overlay is visible"
    );
}

#[test]
fn open_tab_ai_chat_guards_save_offer_state() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("tab_ai_save_offer_state.is_some()"),
        "open_tab_ai_chat must guard against opening while save-offer is visible"
    );
}

// =========================================================================
// Harness terminal: context injection and capture profile
// =========================================================================

#[test]
fn harness_terminal_uses_text_safe_capture_profile() {
    // The deferred capture pipeline must use tab_ai_submit() (no screenshots
    // in the blob) not tab_ai() (includes base64 screenshots that bloat PTY).
    let capture_fn_start = TAB_AI_MODE_SOURCE
        .find("fn spawn_tab_ai_pre_switch_capture(")
        .expect("spawn_tab_ai_pre_switch_capture must exist");
    let capture_fn_body = &TAB_AI_MODE_SOURCE[capture_fn_start..];
    let next_fn = capture_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(capture_fn_body.len());
    let capture_fn_body = &capture_fn_body[..next_fn];

    assert!(
        capture_fn_body.contains("tab_ai_submit()"),
        "deferred capture must use tab_ai_submit() (text-safe, no screenshots) for generic PTY"
    );
    assert!(
        !capture_fn_body.contains("CaptureContextOptions::tab_ai()"),
        "deferred capture must NOT use tab_ai() (includes screenshots) for generic PTY backends"
    );
}

#[test]
fn harness_terminal_entry_derives_mode_from_intent() {
    let open_fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_tab_ai_harness_terminal_from_request(")
        .expect("open_tab_ai_harness_terminal_from_request must exist");
    let open_fn_body = &TAB_AI_MODE_SOURCE[open_fn_start..];
    let next_fn = open_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(open_fn_body.len());
    let open_fn_body = &open_fn_body[..next_fn];

    assert!(
        open_fn_body.contains("PasteOnly"),
        "Zero-intent Tab entry must use PasteOnly so context is staged without auto-submitting"
    );
    assert!(
        open_fn_body.contains("let submit_now = request"),
        "agent_handoff.rs must derive submit flag from quick_submit_plan or entry_intent"
    );
    assert!(
        open_fn_body.contains("let submission_mode = if submit_now"),
        "agent_handoff.rs must derive submission mode from resolved submit flag"
    );
    assert!(
        open_fn_body.contains("TabAiHarnessSubmissionMode::Submit"),
        "Typed Tab entry must submit immediately through the harness"
    );
    assert!(
        open_fn_body.contains("effective_intent"),
        "Typed Tab entry must prefer quick-submit synthesized intent over raw entry_intent"
    );
}

#[test]
fn harness_injection_supports_paste_only_mode() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("send_text_as_paste"),
        "PasteOnly injection must use send_text_as_paste, not send_line"
    );
}

#[test]
fn harness_submission_uses_script_kit_context_block() {
    assert!(
        HARNESS_SOURCE.contains("Script Kit context"),
        "harness submission must contain the flat labeled context header"
    );
}

// =========================================================================
// Harness session lifecycle: reuse and validation
// =========================================================================

#[test]
fn ensure_harness_reuses_existing_live_session() {
    // ensure_tab_ai_harness_terminal must check for an existing live session
    // and return it without spawning a new PTY.
    let ensure_fn_start = TAB_AI_MODE_SOURCE
        .find("fn ensure_tab_ai_harness_terminal(")
        .expect("ensure_tab_ai_harness_terminal must exist");
    let ensure_fn_body = &TAB_AI_MODE_SOURCE[ensure_fn_start..];
    let next_fn = ensure_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(ensure_fn_body.len());
    let ensure_fn_body = &ensure_fn_body[..next_fn];

    assert!(
        ensure_fn_body.contains("tab_ai_harness"),
        "must check the existing harness session"
    );
    assert!(
        ensure_fn_body.contains("is_alive()"),
        "must check whether the existing session is still alive"
    );
    assert!(
        ensure_fn_body.contains("false"),
        "reused session must return was_cold_start=false"
    );
}

#[test]
fn harness_startup_validates_config_before_spawn() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("validate_tab_ai_harness_config"),
        "ensure_tab_ai_harness_terminal must validate config before spawning the PTY"
    );
}

#[test]
fn harness_validation_checks_empty_command() {
    let config = script_kit_gpui::ai::HarnessConfig {
        command: "".to_string(),
        ..Default::default()
    };
    let err = script_kit_gpui::ai::validate_tab_ai_harness_config(&config)
        .expect_err("empty command must fail");
    assert!(
        err.contains("config.ts"),
        "error must mention config file path for setup guidance: {err}"
    );
}

#[test]
fn harness_validation_checks_missing_cli() {
    let config = script_kit_gpui::ai::HarnessConfig {
        command: "nonexistent-harness-cli-abc123".to_string(),
        ..Default::default()
    };
    let err = script_kit_gpui::ai::validate_tab_ai_harness_config(&config)
        .expect_err("missing CLI must fail");
    assert!(
        err.contains("not found on PATH"),
        "error must mention PATH: {err}"
    );
    assert!(
        err.contains("config.ts"),
        "error must mention config file for setup guidance: {err}"
    );
}

#[test]
fn harness_startup_failure_toast_mentions_setup_guidance() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("config.ts"),
        "harness startup failure toast must mention the config file for user guidance"
    );
}

// =========================================================================
// Close semantics: Cmd+W closes wrapper, Escape owned by PTY
// =========================================================================

#[test]
fn quick_terminal_cmd_w_closes_main_window_state_first() {
    assert!(
        TERM_RENDER_SOURCE.contains("this.close_quick_terminal_main_window_state_first(cx);"),
        "Cmd+W in QuickTerminalView must use the state-first main-window close path"
    );
    assert!(
        !TERM_RENDER_SOURCE.contains(
            "key.eq_ignore_ascii_case(\"w\")\n                        {\n                            this.close_tab_ai_harness_terminal_with_window"
        ),
        "Cmd+W must not use the visible restore-origin close helper"
    );
}

#[test]
fn quick_terminal_close_semantics_documented_in_code() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("Cmd+W` closes the Quick Terminal main window state-first"),
        "harness close docs must state Cmd+W closes Quick Terminal state-first"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("Escape` is forwarded to the PTY"),
        "close_tab_ai_harness_terminal doc must state Escape goes to the PTY"
    );
}

#[test]
fn quick_terminal_render_documents_close_contract() {
    assert!(
        TERM_RENDER_SOURCE.contains("wrapper semantics contract"),
        "render_prompts/term.rs must document the wrapper semantics contract"
    );
    assert!(
        TERM_RENDER_SOURCE.contains("Cmd+W closes the main window state-first"),
        "render_prompts/term.rs must state Cmd+W closes state-first"
    );
    assert!(
        TERM_RENDER_SOURCE.contains("Escape is forwarded to the PTY"),
        "render_prompts/term.rs must state Escape goes to the PTY"
    );
}

#[test]
fn app_state_tracks_tab_ai_harness_return_target() {
    assert!(
        APP_STATE_SOURCE.contains("tab_ai_harness_return_view"),
        "app_state.rs must store the view to restore when leaving the Tab AI terminal"
    );
    assert!(
        APP_STATE_SOURCE.contains("tab_ai_harness_return_focus_target"),
        "app_state.rs must store the focus target to restore when leaving the Tab AI terminal"
    );
}

#[test]
fn tab_ai_reentry_uses_saved_originating_view() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("tab_ai_harness_return_view"),
        "warm-session reentry must use the saved originating view, not QuickTerminalView"
    );
}

#[test]
fn close_tab_ai_harness_terminal_clears_cached_session() {
    let close_fn_body = source_block_after(
        TAB_AI_MODE_SOURCE,
        "fn close_tab_ai_harness_terminal_impl(",
        4500,
    );

    assert!(
        close_fn_body.contains("self.terminate_tab_ai_harness_session(cx);"),
        "close must clear cached harness session through terminate_tab_ai_harness_session()"
    );
    assert!(
        close_fn_body.contains("self.tab_ai_harness_apply_back_route = None;"),
        "close must clear the cached apply-back route"
    );
    assert!(
        close_fn_body.contains("tab_ai_harness_capture_generation += 1"),
        "close must invalidate in-flight deferred capture generations"
    );
    assert!(
        close_fn_body.contains("schedule_tab_ai_harness_prewarm"),
        "close must schedule a fresh prewarm after teardown via schedule_tab_ai_harness_prewarm"
    );
}

#[test]
fn close_tab_ai_harness_logs_session_cleared() {
    let close_fn_body = source_block_after(
        TAB_AI_MODE_SOURCE,
        "fn close_tab_ai_harness_terminal_impl(",
        4500,
    );

    assert!(
        close_fn_body.contains("session_cleared = closing_quick_terminal"),
        "close log must report whether the PTY-backed session was cleared"
    );
}

#[test]
fn term_prompt_exposes_terminate_session() {
    let term_source = include_str!("../src/term_prompt/mod.rs");
    assert!(
        term_source.contains("pub fn terminate_session(&mut self) -> anyhow::Result<()>"),
        "TermPrompt must expose a public terminate_session method"
    );
    // Must not use unwrap/expect in production code
    let term_fn_start = term_source
        .find("pub fn terminate_session(")
        .expect("terminate_session must exist");
    let term_fn_body = &term_source[term_fn_start..];
    let next_fn = term_fn_body[1..]
        .find("\n    pub fn ")
        .unwrap_or(term_fn_body.len());
    let term_fn_body = &term_fn_body[..next_fn];

    assert!(
        !term_fn_body.contains(".unwrap()") && !term_fn_body.contains(".expect("),
        "terminate_session must not use unwrap/expect in production code"
    );
}

#[test]
fn schedule_tab_ai_harness_prewarm_exists_and_uses_timer() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("fn schedule_tab_ai_harness_prewarm("),
        "schedule_tab_ai_harness_prewarm helper must exist"
    );
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn schedule_tab_ai_harness_prewarm(")
        .expect("schedule_tab_ai_harness_prewarm must exist");
    let fn_body = &TAB_AI_MODE_SOURCE[fn_start..];
    let next_fn = fn_body[1..]
        .find("\n    fn ")
        .or_else(|| fn_body[1..].find("\n    pub"))
        .unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_fn];

    assert!(
        fn_body.contains("background_executor().timer("),
        "prewarm scheduler must use background_executor timer for the delay"
    );
    // The scheduler delegates to the post-close prewarm helper, which
    // internally calls warm_tab_ai_harness_silently.
    assert!(
        fn_body.contains("warm_tab_ai_harness_after_close"),
        "prewarm scheduler must delegate to warm_tab_ai_harness_after_close"
    );
    assert!(
        fn_body.contains(".detach()"),
        "prewarm task must be detached"
    );
}

// =========================================================================
// Overlay state is fully removed
// =========================================================================

#[test]
fn app_state_no_longer_contains_tab_ai_overlay_state() {
    assert!(
        !APP_STATE_SOURCE.contains("tab_ai_state: Option<TabAiOverlayState>"),
        "legacy overlay state field must be removed from app_state.rs"
    );
}

#[test]
fn app_view_state_no_longer_declares_tab_ai_overlay_state() {
    assert!(
        !APP_VIEW_STATE_SOURCE.contains("struct TabAiOverlayState"),
        "legacy TabAiOverlayState struct must be deleted from app_view_state.rs"
    );
}

#[test]
fn agent_handoff_no_longer_renders_overlay() {
    assert!(
        !TAB_AI_MODE_SOURCE.contains("render_tab_ai_overlay"),
        "Tab AI should route through the harness terminal, not the overlay"
    );
}

#[test]
fn agent_handoff_no_longer_has_overlay_open_close() {
    assert!(
        !TAB_AI_MODE_SOURCE.contains("fn open_tab_ai_overlay"),
        "open_tab_ai_overlay must be removed"
    );
    assert!(
        !TAB_AI_MODE_SOURCE.contains("fn close_tab_ai_overlay"),
        "close_tab_ai_overlay must be removed"
    );
    assert!(
        !TAB_AI_MODE_SOURCE.contains("fn is_tab_ai_overlay_open"),
        "is_tab_ai_overlay_open must be removed"
    );
}

// =========================================================================
// Save-offer overlay rendering (still active)
// =========================================================================

#[test]
fn render_impl_renders_tab_ai_save_offer_overlay() {
    assert!(
        RENDER_IMPL_SOURCE.contains("render_tab_ai_save_offer_overlay"),
        "render_impl.rs must build the Tab AI save-offer overlay"
    );
    assert!(
        RENDER_IMPL_SOURCE.contains("tab_ai_save_offer_overlay"),
        "render_impl.rs must compose the Tab AI save-offer overlay into the overlay stack"
    );
}

// =========================================================================
// ScriptList fallback routes through the shared Cmd+Enter AI helper
// =========================================================================

#[test]
fn script_list_cmd_enter_fallback_routes_to_shared_agent_chat_helper() {
    assert!(
        SCRIPT_LIST_SOURCE.contains("try_route_global_cmd_enter_to_agent_chat_context_capture(cx)"),
        "ScriptList Cmd+Enter fallback must route through the shared AI helper"
    );
    assert!(
        !SCRIPT_LIST_SOURCE.contains("open_tab_ai_overlay"),
        "ScriptList Cmd+Enter fallback must not reference the removed overlay"
    );
    assert!(
        !SCRIPT_LIST_SOURCE.contains("open_ai_chat_from_main_window_query"),
        "ScriptList Cmd+Enter fallback must not reopen the old inline AI chat path"
    );
}

// =========================================================================
// ScriptList fallback comment matches Cmd+Enter contract
// =========================================================================

#[test]
fn deterministic_context_capture_function_is_public() {
    let capture_source = include_str!("../src/context_snapshot/capture.rs");
    assert!(
        capture_source.contains("pub fn enable_deterministic_context_capture()"),
        "enable_deterministic_context_capture must be a public function for integration tests"
    );
}

#[test]
fn deterministic_capture_uses_atomic_flag() {
    let capture_source = include_str!("../src/context_snapshot/capture.rs");
    assert!(
        capture_source.contains("DETERMINISTIC_CONTEXT"),
        "deterministic capture must use a static atomic flag"
    );
    assert!(
        capture_source.contains("AtomicBool"),
        "the flag must be an AtomicBool for thread safety"
    );
}

#[test]
fn live_capture_respects_deterministic_flag() {
    let capture_source = include_str!("../src/context_snapshot/capture.rs");
    let capture_fn_start = capture_source
        .find("pub fn capture_context_snapshot(")
        .expect("capture_context_snapshot must exist");
    let capture_fn_body = &capture_source[capture_fn_start..];
    let next_fn = capture_fn_body[1..]
        .find("\npub ")
        .unwrap_or(capture_fn_body.len());
    let capture_fn_body = &capture_fn_body[..next_fn];

    assert!(
        capture_fn_body.contains("DETERMINISTIC_CONTEXT"),
        "capture_context_snapshot must check the deterministic flag to return empty seed in tests"
    );
}

// =========================================================================
// Legacy inline chat: retained but NOT the primary Tab entry
// =========================================================================

#[test]
fn implicit_target_detection_uses_public_function() {
    let tab_context_source = include_str!("../src/ai/tab_context.rs");
    assert!(
        tab_context_source.contains("pub fn tab_ai_intent_uses_implicit_target("),
        "tab_ai_intent_uses_implicit_target must be a public function"
    );
}

// =========================================================================
// Warm startup: prewarm on startup, opt-out, and first-Tab reuse
// =========================================================================

#[test]
fn startup_wires_prewarm_call() {
    // The startup file must call warm_tab_ai_harness_on_startup asynchronously
    // so the first Tab press finds a live PTY.
    assert!(
        TAB_SOURCE.contains("warm_tab_ai_harness_on_startup"),
        "startup.rs must invoke warm_tab_ai_harness_on_startup"
    );
}

#[test]
fn startup_prewarm_runs_after_tab_interceptor() {
    // Prewarm must be wired AFTER the Tab interceptor subscription is pushed,
    // so the interceptor is guaranteed to be in place before the harness spawns.
    let interceptor_push_pos = TAB_SOURCE
        .find("gpui_input_subscriptions.push(tab_interceptor)")
        .expect("Tab interceptor push must exist");
    let prewarm_pos = TAB_SOURCE
        .find("warm_tab_ai_harness_on_startup")
        .expect("prewarm call must exist");

    assert!(
        interceptor_push_pos < prewarm_pos,
        "prewarm must be wired after the Tab interceptor is installed"
    );
}

#[test]
fn startup_prewarm_is_async_and_detached() {
    // Prewarm must not block the startup path — it should be a spawned task.
    let prewarm_pos = TAB_SOURCE
        .find("warm_tab_ai_harness_on_startup")
        .expect("prewarm call must exist");
    let after_prewarm = &TAB_SOURCE[prewarm_pos..];

    assert!(
        after_prewarm.contains(".detach()"),
        "prewarm task must be detached so it does not block startup"
    );
}

#[test]
fn prewarm_checks_warm_on_startup_flag() {
    // The warm_on_startup check lives in warm_tab_ai_harness_silently,
    // which warm_tab_ai_harness_on_startup delegates to.
    let warm_fn_start = TAB_AI_MODE_SOURCE
        .find("fn warm_tab_ai_harness_silently(")
        .expect("warm_tab_ai_harness_silently must exist");
    let warm_fn_body = &TAB_AI_MODE_SOURCE[warm_fn_start..];
    let next_fn = warm_fn_body[1..]
        .find("\n    fn ")
        .or_else(|| warm_fn_body[1..].find("\n    pub"))
        .unwrap_or(warm_fn_body.len());
    let warm_fn_body = &warm_fn_body[..next_fn];

    assert!(
        warm_fn_body.contains("warm_on_startup"),
        "prewarm must check config.warm_on_startup before spawning"
    );
    assert!(
        warm_fn_body.contains("\"disabled\""),
        "prewarm must log reason=disabled when warm_on_startup is false"
    );
}

#[test]
fn prewarm_reuses_ensure_harness_terminal() {
    // Prewarm must call the same session constructor that Tab uses.
    // The logic lives in warm_tab_ai_harness_silently, which both
    // warm_tab_ai_harness_on_startup and warm_tab_ai_harness_after_close delegate to.
    let warm_fn_start = TAB_AI_MODE_SOURCE
        .find("fn warm_tab_ai_harness_silently(")
        .expect("warm_tab_ai_harness_silently must exist");
    let warm_fn_body = &TAB_AI_MODE_SOURCE[warm_fn_start..];
    let next_fn = warm_fn_body[1..]
        .find("\n    fn ")
        .or_else(|| warm_fn_body[1..].find("\n    pub"))
        .unwrap_or(warm_fn_body.len());
    let warm_fn_body = &warm_fn_body[..next_fn];

    assert!(
        warm_fn_body.contains("ensure_tab_ai_harness_terminal"),
        "prewarm must call ensure_tab_ai_harness_terminal (same path as Tab)"
    );
}

#[test]
fn prewarm_skips_if_session_already_alive() {
    // If a harness session is already live, prewarm should no-op.
    // The logic lives in warm_tab_ai_harness_silently.
    let warm_fn_start = TAB_AI_MODE_SOURCE
        .find("fn warm_tab_ai_harness_silently(")
        .expect("warm_tab_ai_harness_silently must exist");
    let warm_fn_body = &TAB_AI_MODE_SOURCE[warm_fn_start..];
    let next_fn = warm_fn_body[1..]
        .find("\n    fn ")
        .or_else(|| warm_fn_body[1..].find("\n    pub"))
        .unwrap_or(warm_fn_body.len());
    let warm_fn_body = &warm_fn_body[..next_fn];

    assert!(
        warm_fn_body.contains("is_alive()"),
        "prewarm must check if existing session is alive"
    );
    assert!(
        warm_fn_body.contains("\"already_alive\""),
        "prewarm must log reason=already_alive when skipping"
    );
}

#[test]
fn prewarm_handles_config_read_failure_silently() {
    // If config cannot be read, prewarm must log and return — no toast, no panic.
    // The logic lives in warm_tab_ai_harness_silently.
    let warm_fn_start = TAB_AI_MODE_SOURCE
        .find("fn warm_tab_ai_harness_silently(")
        .expect("warm_tab_ai_harness_silently must exist");
    let warm_fn_body = &TAB_AI_MODE_SOURCE[warm_fn_start..];
    let next_fn = warm_fn_body[1..]
        .find("\n    fn ")
        .or_else(|| warm_fn_body[1..].find("\n    pub"))
        .unwrap_or(warm_fn_body.len());
    let warm_fn_body = &warm_fn_body[..next_fn];

    assert!(
        warm_fn_body.contains("\"config_read_failed\""),
        "prewarm must log reason=config_read_failed on I/O error"
    );
    // Must not contain toast/hud/user-facing error calls
    assert!(
        !warm_fn_body.contains("show_hud"),
        "prewarm must NOT show HUD on config failure"
    );
    assert!(
        !warm_fn_body.contains("toast_manager"),
        "prewarm must NOT show toast on config failure"
    );
}

#[test]
fn prewarm_handles_invalid_config_silently() {
    // If config is invalid (e.g. missing binary), prewarm must log and return.
    // The logic lives in warm_tab_ai_harness_silently.
    let warm_fn_start = TAB_AI_MODE_SOURCE
        .find("fn warm_tab_ai_harness_silently(")
        .expect("warm_tab_ai_harness_silently must exist");
    let warm_fn_body = &TAB_AI_MODE_SOURCE[warm_fn_start..];
    let next_fn = warm_fn_body[1..]
        .find("\n    fn ")
        .or_else(|| warm_fn_body[1..].find("\n    pub"))
        .unwrap_or(warm_fn_body.len());
    let warm_fn_body = &warm_fn_body[..next_fn];

    assert!(
        warm_fn_body.contains("validate_tab_ai_harness_config"),
        "prewarm must validate config before spawning"
    );
    assert!(
        warm_fn_body.contains("\"invalid_config\""),
        "prewarm must log reason=invalid_config on validation failure"
    );
}

#[test]
fn prewarm_logs_structured_events() {
    // All prewarm paths must use tracing with structured event fields
    // so SCRIPT_KIT_AI_LOG=1 output is machine-parseable.
    // The logic lives in warm_tab_ai_harness_silently.
    let warm_fn_start = TAB_AI_MODE_SOURCE
        .find("fn warm_tab_ai_harness_silently(")
        .expect("warm_tab_ai_harness_silently must exist");
    let warm_fn_body = &TAB_AI_MODE_SOURCE[warm_fn_start..];
    let next_fn = warm_fn_body[1..]
        .find("\n    fn ")
        .or_else(|| warm_fn_body[1..].find("\n    pub"))
        .unwrap_or(warm_fn_body.len());
    let warm_fn_body = &warm_fn_body[..next_fn];

    assert!(
        warm_fn_body.contains("event = \"tab_ai_harness_prewarmed\""),
        "success path must log event=tab_ai_harness_prewarmed"
    );
    assert!(
        warm_fn_body.contains("event = \"tab_ai_harness_prewarm_failed\""),
        "failure path must log event=tab_ai_harness_prewarm_failed"
    );
    assert!(
        warm_fn_body.contains("event = \"tab_ai_harness_prewarm_skipped\""),
        "skip paths must log event=tab_ai_harness_prewarm_skipped"
    );
}

#[test]
fn prewarm_does_not_switch_view() {
    // Prewarm must be invisible to the user — no view transition.
    let warm_fn_start = TAB_AI_MODE_SOURCE
        .find("fn warm_tab_ai_harness_on_startup(")
        .expect("warm_tab_ai_harness_on_startup must exist");
    let warm_fn_body = &TAB_AI_MODE_SOURCE[warm_fn_start..];
    let next_fn = warm_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(warm_fn_body.len());
    let warm_fn_body = &warm_fn_body[..next_fn];

    assert!(
        !warm_fn_body.contains("current_view"),
        "prewarm must NOT change current_view — it is invisible to the user"
    );
    assert!(
        !warm_fn_body.contains("QuickTerminalView"),
        "prewarm must NOT switch to QuickTerminalView"
    );
}

// =========================================================================
// Post-close prewarm lifecycle: close clears session, prewarm seeds startup,
// explicit Tab still forces a fresh PTY
// =========================================================================

#[test]
fn prewarm_tags_cold_start_as_fresh_prewarm() {
    // After a cold-start prewarm, the session must be tagged FreshPrewarm.
    // The logic lives in warm_tab_ai_harness_silently.
    let warm_fn_start = TAB_AI_MODE_SOURCE
        .find("fn warm_tab_ai_harness_silently(")
        .expect("warm_tab_ai_harness_silently must exist");
    let warm_fn_body = &TAB_AI_MODE_SOURCE[warm_fn_start..];
    let next_fn = warm_fn_body[1..]
        .find("\n    fn ")
        .or_else(|| warm_fn_body[1..].find("\n    pub"))
        .unwrap_or(warm_fn_body.len());
    let warm_fn_body = &warm_fn_body[..next_fn];

    assert!(
        warm_fn_body.contains("mark_fresh_prewarm"),
        "prewarm must tag cold-start sessions via mark_fresh_prewarm()"
    );
    assert!(
        warm_fn_body.contains("was_cold_start"),
        "prewarm must only tag FreshPrewarm when was_cold_start is true"
    );
}

#[test]
fn close_then_prewarm_then_reuse_once_lifecycle_contract() {
    // End-to-end contract: close clears session → prewarm seeds a warm PTY
    // → first explicit Tab reuses the warm PTY once → later opens force fresh.

    // 1. close_tab_ai_harness_terminal clears session and schedules prewarm
    let close_fn_start = TAB_AI_MODE_SOURCE
        .find("fn close_tab_ai_harness_terminal(")
        .expect("close_tab_ai_harness_terminal must exist");
    let close_fn_body = &TAB_AI_MODE_SOURCE[close_fn_start..];
    let next_fn = close_fn_body[1..]
        .find("\n    fn ")
        .or_else(|| close_fn_body[1..].find("\n    pub"))
        .unwrap_or(close_fn_body.len());
    let close_fn_body = &close_fn_body[..next_fn];

    assert!(
        close_fn_body.contains("terminate_tab_ai_harness_session"),
        "close must clear the harness session through terminate_tab_ai_harness_session"
    );
    assert!(
        close_fn_body.contains("schedule_tab_ai_harness_prewarm"),
        "close must schedule a deferred prewarm"
    );

    // 2. warm_tab_ai_harness_silently tags cold starts as FreshPrewarm
    assert!(
        TAB_AI_MODE_SOURCE.contains("mark_fresh_prewarm()"),
        "prewarm path must tag cold-start sessions via mark_fresh_prewarm()"
    );

    // 3. open path reuses a fresh prewarm once, then forces fresh
    let open_fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_tab_ai_harness_terminal_from_request(")
        .expect("open_tab_ai_harness_terminal_from_request must exist");
    let open_fn_body = &TAB_AI_MODE_SOURCE[open_fn_start..];
    let next_fn = open_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(open_fn_body.len());
    let open_fn_body = &open_fn_body[..next_fn];

    assert!(
        open_fn_body.contains("is_fresh_prewarm"),
        "open path must check for a fresh silently-prewarmed session"
    );
    assert!(
        open_fn_body.contains("mark_consumed"),
        "open path must consume the prewarm once"
    );
    assert!(
        open_fn_body.contains("!reuse_fresh_prewarm"),
        "open path must conditionally reuse: !reuse_fresh_prewarm → force_fresh"
    );
}

#[test]
fn warm_state_enum_has_both_variants() {
    // TabAiHarnessWarmState must exist in the harness source with both variants.
    assert!(
        HARNESS_SOURCE.contains("enum TabAiHarnessWarmState"),
        "TabAiHarnessWarmState enum must exist in harness/mod.rs"
    );
    assert!(
        HARNESS_SOURCE.contains("FreshPrewarm"),
        "TabAiHarnessWarmState must have FreshPrewarm variant"
    );
    assert!(
        HARNESS_SOURCE.contains("Consumed"),
        "TabAiHarnessWarmState must have Consumed variant"
    );
}

#[test]
fn session_state_has_warm_state_field() {
    // TabAiHarnessSessionState must include the warm_state field.
    assert!(
        HARNESS_SOURCE.contains("warm_state: TabAiHarnessWarmState"),
        "TabAiHarnessSessionState must have a warm_state field of type TabAiHarnessWarmState"
    );
}

#[test]
fn harness_config_default_warm_on_startup_is_true() {
    // The default config must have warm_on_startup=true so prewarm is opt-out.
    let config = script_kit_gpui::ai::HarnessConfig::default();
    assert!(
        config.warm_on_startup,
        "HarnessConfig::default() must have warm_on_startup=true"
    );
}

#[test]
fn harness_config_missing_warm_field_deserializes_as_true() {
    // Old config files without warmOnStartup should default to true.
    let json = r#"{"schemaVersion":1,"backend":"claudeCode","command":"claude"}"#;
    let config: script_kit_gpui::ai::HarnessConfig =
        serde_json::from_str(json).expect("deserialize");
    assert!(
        config.warm_on_startup,
        "missing warmOnStartup must default to true"
    );
}

#[test]
fn harness_config_explicit_opt_out_deserializes_as_false() {
    // Explicit "warmOnStartup": false must be respected.
    let json =
        r#"{"schemaVersion":1,"backend":"claudeCode","command":"claude","warmOnStartup":false}"#;
    let config: script_kit_gpui::ai::HarnessConfig =
        serde_json::from_str(json).expect("deserialize");
    assert!(
        !config.warm_on_startup,
        "explicit warmOnStartup=false must be preserved"
    );
}

// =========================================================================
// Harness injection: cold-start readiness gate
// =========================================================================

#[test]
fn harness_injection_uses_readiness_gate_for_cold_start() {
    let inject_fn_start = TAB_AI_MODE_SOURCE
        .find("fn inject_tab_ai_harness_submission(")
        .expect("inject_tab_ai_harness_submission must exist");
    let inject_fn_body = &TAB_AI_MODE_SOURCE[inject_fn_start..];
    let next_fn = inject_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(inject_fn_body.len());
    let inject_fn_body = &inject_fn_body[..next_fn];

    assert!(
        inject_fn_body.contains("wait_for_readiness"),
        "injection must accept a readiness flag independent of cold/warm start"
    );
    assert!(
        inject_fn_body.contains("has_received_output"),
        "cold-start injection must poll has_received_output as a readiness signal"
    );
    assert!(
        inject_fn_body.contains("HARNESS_READINESS_TIMEOUT_MS"),
        "readiness gate must use a bounded timeout constant"
    );
    // Must NOT use a fixed sleep as the sole delay mechanism
    assert!(
        !inject_fn_body.contains("let delay_ms"),
        "injection must not use a fixed delay_ms variable — use readiness polling instead"
    );
}

#[test]
fn harness_readiness_gate_has_bounded_timeout() {
    // The readiness gate must define both a timeout and a poll interval
    // as associated constants, ensuring they are not magic numbers.
    assert!(
        TAB_AI_MODE_SOURCE.contains("HARNESS_READINESS_TIMEOUT_MS"),
        "must define HARNESS_READINESS_TIMEOUT_MS constant"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("HARNESS_READINESS_POLL_MS"),
        "must define HARNESS_READINESS_POLL_MS constant"
    );
}

#[test]
fn harness_readiness_gate_logs_timeout_on_fallback() {
    // When the harness does not produce output within the timeout,
    // the gate must log a warning so slow startups are diagnosable.
    let inject_fn_start = TAB_AI_MODE_SOURCE
        .find("fn inject_tab_ai_harness_submission(")
        .expect("inject_tab_ai_harness_submission must exist");
    let inject_fn_body = &TAB_AI_MODE_SOURCE[inject_fn_start..];
    let next_fn = inject_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(inject_fn_body.len());
    let inject_fn_body = &inject_fn_body[..next_fn];

    assert!(
        inject_fn_body.contains("tab_ai_harness_readiness_timeout"),
        "readiness gate must log a warning event on timeout fallback"
    );
}

#[test]
fn term_prompt_exposes_has_received_output_field() {
    // TermPrompt must have a public `has_received_output` field that the
    // readiness gate can poll.
    let term_source = include_str!("../src/term_prompt/mod.rs");
    assert!(
        term_source.contains("pub has_received_output: bool"),
        "TermPrompt must expose has_received_output as a public bool field"
    );
    // Must be initialized to false
    assert!(
        term_source.contains("has_received_output: false"),
        "has_received_output must be initialized to false"
    );
    // Must be set to true when output is received
    assert!(
        term_source.contains("has_received_output = true"),
        "has_received_output must be set to true when the PTY produces output"
    );
}

#[test]
fn harness_injection_submits_or_pastes_based_on_flag() {
    let inject_fn_start = TAB_AI_MODE_SOURCE
        .find("fn inject_tab_ai_harness_submission(")
        .expect("inject_tab_ai_harness_submission must exist");
    let inject_fn_body = &TAB_AI_MODE_SOURCE[inject_fn_start..];
    let next_fn = inject_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(inject_fn_body.len());
    let inject_fn_body = &inject_fn_body[..next_fn];

    assert!(
        inject_fn_body.contains("send_line"),
        "Submit mode must use send_line for a full turn"
    );
    assert!(
        inject_fn_body.contains("send_text_as_paste"),
        "PasteOnly mode must use send_text_as_paste for staging"
    );
}

// =========================================================================
// Harness readiness gate: prewarmed sessions also wait for output
// =========================================================================

#[test]
fn harness_readiness_gate_applies_to_prewarmed_sessions() {
    // A reused (prewarmed) session with has_received_output == false must
    // still take the readiness-wait path before context paste. The gate
    // must NOT be conditioned on was_cold_start alone.
    let open_fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_tab_ai_harness_terminal_from_request(")
        .expect("open_tab_ai_harness_terminal_from_request must exist");
    let open_fn_body = &TAB_AI_MODE_SOURCE[open_fn_start..];
    let next_fn = open_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(open_fn_body.len());
    let open_fn_body = &open_fn_body[..next_fn];

    // The open function must call the readiness check method
    assert!(
        open_fn_body.contains("tab_ai_harness_needs_readiness_wait"),
        "open_tab_ai_harness_terminal_from_request must call tab_ai_harness_needs_readiness_wait \
         to determine readiness independent of cold-start status"
    );

    // The readiness check must be based on has_received_output, not was_cold_start
    assert!(
        open_fn_body.contains("wait_for_readiness"),
        "open_tab_ai_harness_terminal_from_request must use wait_for_readiness from the readiness check"
    );
}

#[test]
fn harness_readiness_check_method_exists_and_checks_output() {
    // tab_ai_harness_needs_readiness_wait must exist and check has_received_output
    assert!(
        TAB_AI_MODE_SOURCE.contains("fn tab_ai_harness_needs_readiness_wait("),
        "tab_ai_harness_needs_readiness_wait method must exist"
    );

    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn tab_ai_harness_needs_readiness_wait(")
        .expect("method must exist");
    let fn_body = &TAB_AI_MODE_SOURCE[fn_start..];
    let next_fn = fn_body[1..].find("\n    fn ").unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_fn];

    assert!(
        fn_body.contains("has_received_output"),
        "readiness check must be based on has_received_output"
    );
}

#[test]
fn harness_injection_uses_wait_for_readiness_not_cold_start() {
    // The injection function must use wait_for_readiness parameter,
    // not was_cold_start, ensuring prewarmed sessions also wait.
    let inject_fn_start = TAB_AI_MODE_SOURCE
        .find("fn inject_tab_ai_harness_submission(")
        .expect("inject_tab_ai_harness_submission must exist");
    let inject_fn_body = &TAB_AI_MODE_SOURCE[inject_fn_start..];
    let next_fn = inject_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(inject_fn_body.len());
    let inject_fn_body = &inject_fn_body[..next_fn];

    assert!(
        inject_fn_body.contains("wait_for_readiness: bool"),
        "injection must accept wait_for_readiness parameter"
    );
    assert!(
        !inject_fn_body.contains("was_cold_start: bool"),
        "injection must NOT use was_cold_start — readiness is output-based"
    );
}

#[test]
fn harness_submission_planned_event_logged() {
    // open_tab_ai_harness_terminal_from_request must log a structured submission_planned event
    // with the wait_for_readiness flag for diagnostics.
    let open_fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_tab_ai_harness_terminal_from_request(")
        .expect("open_tab_ai_harness_terminal_from_request must exist");
    let open_fn_body = &TAB_AI_MODE_SOURCE[open_fn_start..];
    let next_fn = open_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(open_fn_body.len());
    let open_fn_body = &open_fn_body[..next_fn];

    assert!(
        open_fn_body.contains("tab_ai_harness_submission_planned"),
        "must log tab_ai_harness_submission_planned event"
    );
    assert!(
        open_fn_body.contains("wait_for_readiness"),
        "submission_planned event must include wait_for_readiness field"
    );
}

// =========================================================================
// Escape pass-through: QuickTerminalView disables escape-cancel
// =========================================================================

#[test]
fn quick_terminal_disables_escape_cancel() {
    // For QuickTerminalView, escape_cancels must be set to false so that
    // Escape is forwarded to the PTY (harness TUI owns Escape for navigation).
    assert!(
        TERM_RENDER_SOURCE.contains("escape_cancels")
            && TERM_RENDER_SOURCE.contains("QuickTerminalView"),
        "render_prompts/term.rs must set escape_cancels based on QuickTerminalView"
    );

    // The escape_cancels assignment must negate the QuickTerminalView flag.
    assert!(
        TERM_RENDER_SOURCE.contains("term.escape_cancels = !is_quick_terminal;"),
        "escape_cancels must be false for QuickTerminalView"
    );
}

#[test]
fn quick_terminal_cmd_w_dispatches_close() {
    // Cmd+W in QuickTerminalView must be intercepted before reaching the PTY
    // and must use the state-first Quick Terminal main-window close helper.
    assert!(
        TERM_RENDER_SOURCE.contains("close_quick_terminal_main_window_state_first"),
        "Cmd+W in QuickTerminalView must dispatch to the state-first close helper"
    );
    // The handler must check for the "w" key specifically
    assert!(
        TERM_RENDER_SOURCE.contains("\"w\""),
        "Cmd+W handler must match the 'w' key"
    );
}

// =========================================================================
// Fresh-line staging: source-level guard against regression
// =========================================================================

#[test]
fn harness_submission_builder_preserves_fresh_line_staging_for_paste_only() {
    let fn_start = HARNESS_SOURCE
        .find("pub fn build_tab_ai_harness_submission(")
        .expect("build_tab_ai_harness_submission must exist");
    let fn_body = &HARNESS_SOURCE[fn_start..];
    let next_section = fn_body[1..]
        .find("\n// ---------------------------------------------------------------------------")
        .map(|idx| idx + 1)
        .unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_section];

    assert!(
        fn_body.contains("if !output.ends_with('\\n') {")
            || fn_body.contains("output.push('\\n');")
            || fn_body.contains("output.push_str(\"\\n\")"),
        "PasteOnly must leave the staged context on a fresh line so the next user keystrokes do not join the context block"
    );
}

// =========================================================================
// Backend smoke matrix: each built-in backend through PasteOnly path
// =========================================================================

/// Helper: build a deterministic context blob for smoke-matrix tests.
fn smoke_matrix_context() -> script_kit_gpui::ai::TabAiContextBlob {
    script_kit_gpui::ai::TabAiContextBlob::from_parts(
        script_kit_gpui::ai::TabAiUiSnapshot {
            prompt_type: "ScriptList".to_string(),
            input_text: Some("test".to_string()),
            ..Default::default()
        },
        Default::default(),
        vec![],
        None,
        vec![],
        vec![],
        "2026-03-29T20:00:00Z".to_string(),
    )
}

/// Helper: build a HarnessConfig for a given backend.
fn config_for_backend(
    backend: script_kit_gpui::ai::HarnessBackendKind,
    command: &str,
) -> script_kit_gpui::ai::HarnessConfig {
    script_kit_gpui::ai::HarnessConfig {
        backend,
        command: command.to_string(),
        ..Default::default()
    }
}

#[test]
fn smoke_matrix_claude_code_paste_only_stages_context() {
    let _config = config_for_backend(
        script_kit_gpui::ai::HarnessBackendKind::ClaudeCode,
        "claude",
    );
    let context = smoke_matrix_context();
    let submission = script_kit_gpui::ai::build_tab_ai_harness_submission(
        &context,
        None,
        script_kit_gpui::ai::TabAiHarnessSubmissionMode::PasteOnly,
        None,
        None,
        &[],
    )
    .expect("Claude Code PasteOnly submission must succeed");

    assert!(submission.contains("Script Kit context"));
    assert!(submission.ends_with('\n'));
    assert!(!submission.contains("Await the user"));
    assert!(!submission.contains("User intent:"));
}

#[test]
fn smoke_matrix_codex_paste_only_stages_context() {
    let _config = config_for_backend(script_kit_gpui::ai::HarnessBackendKind::Codex, "codex");
    let context = smoke_matrix_context();
    let submission = script_kit_gpui::ai::build_tab_ai_harness_submission(
        &context,
        None,
        script_kit_gpui::ai::TabAiHarnessSubmissionMode::PasteOnly,
        None,
        None,
        &[],
    )
    .expect("Codex PasteOnly submission must succeed");

    assert!(submission.contains("Script Kit context"));
    assert!(submission.ends_with('\n'));
    assert!(!submission.contains("Await the user"));
}

#[test]
fn smoke_matrix_agy_paste_only_stages_context() {
    let _config = config_for_backend(script_kit_gpui::ai::HarnessBackendKind::Agy, "agy");
    let context = smoke_matrix_context();
    let submission = script_kit_gpui::ai::build_tab_ai_harness_submission(
        &context,
        None,
        script_kit_gpui::ai::TabAiHarnessSubmissionMode::PasteOnly,
        None,
        None,
        &[],
    )
    .expect("AGY PasteOnly submission must succeed");

    assert!(submission.contains("Script Kit context"));
    assert!(submission.ends_with('\n'));
    assert!(!submission.contains("Await the user"));
}

#[test]
fn smoke_matrix_copilot_cli_paste_only_stages_context() {
    let _config = config_for_backend(script_kit_gpui::ai::HarnessBackendKind::CopilotCli, "gh");
    let context = smoke_matrix_context();
    let submission = script_kit_gpui::ai::build_tab_ai_harness_submission(
        &context,
        None,
        script_kit_gpui::ai::TabAiHarnessSubmissionMode::PasteOnly,
        None,
        None,
        &[],
    )
    .expect("Copilot CLI PasteOnly submission must succeed");

    assert!(submission.contains("Script Kit context"));
    assert!(submission.ends_with('\n'));
    assert!(!submission.contains("Await the user"));
}

#[test]
fn smoke_matrix_all_backends_produce_identical_context_block() {
    // The context block must be backend-agnostic — all four produce the same output.
    let context = smoke_matrix_context();
    let mode = script_kit_gpui::ai::TabAiHarnessSubmissionMode::PasteOnly;

    let claude =
        script_kit_gpui::ai::build_tab_ai_harness_submission(&context, None, mode, None, None, &[])
            .expect("Claude Code");
    let codex =
        script_kit_gpui::ai::build_tab_ai_harness_submission(&context, None, mode, None, None, &[])
            .expect("Codex");
    let agy =
        script_kit_gpui::ai::build_tab_ai_harness_submission(&context, None, mode, None, None, &[])
            .expect("AGY");
    let copilot =
        script_kit_gpui::ai::build_tab_ai_harness_submission(&context, None, mode, None, None, &[])
            .expect("Copilot CLI");

    assert_eq!(
        claude, codex,
        "Claude Code and Codex must produce identical context"
    );
    assert_eq!(codex, agy, "Codex and AGY must produce identical context");
    assert_eq!(
        agy, copilot,
        "AGY and Copilot CLI must produce identical context"
    );
}

#[test]
fn smoke_matrix_submit_mode_appends_sentinel_for_all_backends() {
    let context = smoke_matrix_context();
    let mode = script_kit_gpui::ai::TabAiHarnessSubmissionMode::Submit;

    for label in ["Claude Code", "Codex", "AGY", "Copilot CLI"] {
        let submission = script_kit_gpui::ai::build_tab_ai_harness_submission(
            &context,
            None,
            mode,
            None,
            None,
            &[],
        )
        .unwrap_or_else(|e| panic!("{label} Submit failed: {e}"));
        assert!(
            submission.contains("Await the user's next terminal input."),
            "{label} Submit mode must append wait sentinel"
        );
    }
}

#[test]
fn smoke_matrix_intent_appends_for_all_backends() {
    let context = smoke_matrix_context();

    for (label, mode) in [
        (
            "PasteOnly",
            script_kit_gpui::ai::TabAiHarnessSubmissionMode::PasteOnly,
        ),
        (
            "Submit",
            script_kit_gpui::ai::TabAiHarnessSubmissionMode::Submit,
        ),
    ] {
        let submission = script_kit_gpui::ai::build_tab_ai_harness_submission(
            &context,
            Some("rename this file"),
            mode,
            None,
            None,
            &[],
        )
        .unwrap_or_else(|e| panic!("{label} with intent failed: {e}"));
        assert!(
            submission.contains("User intent:\nrename this file"),
            "{label} with intent must include the user's intent text"
        );
        assert!(
            !submission.contains("Await the user"),
            "{label} with intent must NOT include wait sentinel"
        );
    }
}

// =========================================================================
// Footer hint strip: QuickTerminalView distinguishes close-only and apply-back
// =========================================================================

#[test]
fn quick_terminal_footer_distinguishes_close_only_and_apply_back_states() {
    assert!(
        TERM_RENDER_SOURCE.contains("fn terminal_return_hint_label(return_view: Option<&AppView>)")
            && TERM_RENDER_SOURCE.contains("None => \"⌘W Close\""),
        "QuickTerminalView without a return target must advertise close-only footer copy"
    );
    assert!(
        TERM_RENDER_SOURCE
            .contains("let can_apply_back = route.is_some() && return_view.is_some();")
            && TERM_RENDER_SOURCE.contains("items.push(\"⌘↩ Apply\".to_string());"),
        "QuickTerminalView with an apply-back route and return target must advertise Apply"
    );
    assert!(
        TERM_RENDER_SOURCE.contains("render_simple_hint_strip(items.join(\" · \"), leading)"),
        "QuickTerminalView footer must use render_simple_hint_strip for minimal chrome"
    );
}

// =========================================================================
// Close function guards: only acts when in QuickTerminalView
// =========================================================================

#[test]
fn close_harness_terminal_guards_current_view() {
    // close_tab_ai_harness_terminal must check that we're actually in
    // QuickTerminalView before restoring — prevent no-op restore from
    // corrupting view state when called from unexpected contexts.
    let close_fn_start = TAB_AI_MODE_SOURCE
        .find("fn close_tab_ai_harness_terminal(")
        .expect("close_tab_ai_harness_terminal must exist");
    let close_fn_body = &TAB_AI_MODE_SOURCE[close_fn_start..];
    let next_fn = close_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(close_fn_body.len());
    let close_fn_body = &close_fn_body[..next_fn];

    assert!(
        close_fn_body.contains("QuickTerminalView"),
        "close must verify current_view is QuickTerminalView before restoring"
    );
    assert!(
        close_fn_body.contains("cx.notify()"),
        "close must call cx.notify() to trigger re-render after view change"
    );
}

// =========================================================================
// Consolidated legacy inline-chat regression guard
// =========================================================================
//
// This guard covers ALL changed harness-entry surfaces in a single test.
// If TabAiChat or open_tab_ai_full_view_chat silently reappears in any
// of these files, the harness-terminal contract is broken.

#[test]
fn legacy_inline_chat_absent_from_all_changed_harness_surfaces() {
    // Every source constant that represents a harness-entry surface must be
    // free of legacy inline-chat references. This is the single authoritative
    // guard for the changed harness-entry surfaces.
    let surfaces: &[(&str, &str)] = &[
        ("startup.rs (Tab interceptor)", TAB_SOURCE),
        ("agent_handoff.rs (orchestration)", TAB_AI_MODE_SOURCE),
        (
            "render_prompts/term.rs (QuickTerminalView renderer)",
            TERM_RENDER_SOURCE,
        ),
        (
            "render_script_list/mod.rs (ScriptList Cmd+Enter fallback)",
            SCRIPT_LIST_SOURCE,
        ),
        ("app_view_state.rs (view enum)", APP_VIEW_STATE_SOURCE),
        ("app_state.rs (shared state)", APP_STATE_SOURCE),
        ("ai/mod.rs (public AI module)", AI_MOD_SOURCE),
    ];

    let legacy_markers = &[
        "TabAiChat",
        "open_tab_ai_full_view_chat",
        "open_tab_ai_overlay",
        "render_tab_ai_overlay",
    ];

    for (label, source) in surfaces {
        for marker in legacy_markers {
            assert!(
                !source.contains(marker),
                "{label} must not contain legacy marker `{marker}` — \
                 the harness terminal is the primary Tab AI surface"
            );
        }
    }
}

// =========================================================================
// ScriptList no longer has a hidden Shift+Tab quick-submit path
// =========================================================================

#[test]
fn startup_tab_interceptor_no_longer_routes_shift_tab_query_into_harness() {
    assert!(
        !TAB_SOURCE.contains("submit_to_current_or_new_tab_ai_harness_from_text"),
        "startup.rs must not keep the hidden Shift+Tab quick-submit planner path"
    );
}

#[test]
fn typed_command_enter_entry_uses_submit_mode_in_harness() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("fn open_tab_ai_chat_with_entry_intent("),
        "agent_handoff.rs must expose an entry-intent-aware harness entry point"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("let submit_now = request"),
        "agent_handoff.rs must derive submit flag from quick_submit_plan or entry_intent"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("crate::ai::TabAiHarnessSubmissionMode::Submit"),
        "typed Command+Enter entry must submit immediately through the harness"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("effective_intent"),
        "typed Tab entry must prefer quick-submit synthesized intent over raw entry_intent"
    );
}

#[test]
fn entry_intent_is_trimmed_and_empty_filtered() {
    // Whitespace-only intents must be treated as None (zero-intent).
    assert!(
        TAB_AI_MODE_SOURCE.contains(".map(|value| value.trim().to_string())"),
        "entry_intent must be trimmed"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains(".filter(|value| !value.is_empty())"),
        "empty trimmed intent must be filtered to None"
    );
}

// =========================================================================
// Legacy AI command redirection: all active AI builtins → harness terminal
// =========================================================================

#[test]
fn generate_script_builtin_routes_to_harness_not_chat_prompt() {
    // The GenerateScript match arm must open the harness terminal, not the legacy ChatPrompt.
    assert!(
        BUILTIN_EXECUTION_SOURCE.contains("ai_generate_script_routed_to_harness"),
        "GenerateScript must use harness routing success label"
    );
    assert!(
        BUILTIN_EXECUTION_SOURCE
            .contains("self.open_tab_ai_chat_with_entry_intent(Some(request), cx);")
            || BUILTIN_EXECUTION_SOURCE
                .contains("self.open_tab_ai_agent_chat_with_entry_intent(None, cx);"),
        "GenerateScript must route through the shared Tab AI entry point"
    );
}

#[test]
fn agent_chat_open_commands_route_to_agent_chat() {
    // OpenAi, MiniAi, NewConversation, ClearConversation must all route to Agent Chat.
    // Scope to the AI command match block.
    let ai_block_start = BUILTIN_EXECUTION_SOURCE
        .find("BuiltInFeature::AiCommand(cmd_type) =>")
        .expect("AiCommand match arm must exist");
    let ai_block = &BUILTIN_EXECUTION_SOURCE
        [ai_block_start..(ai_block_start + 4000).min(BUILTIN_EXECUTION_SOURCE.len())];

    assert!(
        !ai_block.contains("clear_all_chats"),
        "AI command block must not call legacy clear_all_chats"
    );
}

#[test]
fn dispatch_ai_script_generation_routes_to_harness() {
    // The legacy dispatch function must now route to the harness, not show_script_generation_chat.
    assert!(
        PROMPT_AI_SOURCE.contains(
            "open_tab_ai_chat_with_entry_intent_suppressing_focused_part(Some(query), cx)",
        ),
        "dispatch_ai_script_generation_from_query must route to harness entry intent while suppressing launcher focus chips"
    );
    // Must NOT call the legacy ChatPrompt path
    let dispatch_fn_start = PROMPT_AI_SOURCE
        .find("fn dispatch_ai_script_generation_from_query(")
        .expect("dispatch function must exist");
    let dispatch_fn_body = &PROMPT_AI_SOURCE[dispatch_fn_start..];
    let next_fn = dispatch_fn_body[1..]
        .find("\n    pub")
        .or_else(|| dispatch_fn_body[1..].find("\n    fn "))
        .unwrap_or(dispatch_fn_body.len());
    let dispatch_fn_body = &dispatch_fn_body[..next_fn];

    assert!(
        !dispatch_fn_body.contains("show_script_generation_chat"),
        "dispatch must not call the legacy show_script_generation_chat"
    );
}

#[test]
fn show_script_generation_chat_is_only_a_harness_shim() {
    // show_script_generation_chat must delegate to the harness entry points
    // and must NOT construct a ChatPrompt or reference ProviderRegistry.
    let fn_start = PROMPT_AI_SOURCE
        .find("pub fn show_script_generation_chat(")
        .expect("show_script_generation_chat must exist");
    let fn_body = &PROMPT_AI_SOURCE[fn_start..];
    let next_fn = fn_body[1..]
        .find("\n    pub")
        .or_else(|| fn_body[1..].find("\n    fn "))
        .unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_fn];

    assert!(
        fn_body.contains("open_tab_ai_chat"),
        "show_script_generation_chat must delegate to open_tab_ai_chat"
    );
    assert!(
        !fn_body.contains("ChatPrompt::new"),
        "show_script_generation_chat must not construct a ChatPrompt"
    );
    assert!(
        !fn_body.contains("ProviderRegistry"),
        "show_script_generation_chat must not reference ProviderRegistry"
    );
    assert!(
        !fn_body.contains("AppView::ChatPrompt"),
        "show_script_generation_chat must not set the view to ChatPrompt"
    );
}

#[test]
fn manual_creation_paths_unaffected_by_ai_redirect() {
    // NewScript and NewExtension must remain registered as non-AI creation paths.
    assert!(
        BUILTINS_SOURCE.contains("ScriptCommandType::NewScript"),
        "NewScript must still be registered"
    );
    assert!(
        BUILTINS_SOURCE.contains("ScriptCommandType::NewExtension"),
        "NewExtension must still be registered"
    );
}

// =========================================================================
// Deferred capture pipeline: "open first, inject later"
// =========================================================================

#[test]
fn harness_terminal_open_path_defers_capture_until_after_view_switch() {
    // open_tab_ai_harness_terminal_from_request must set AppView::QuickTerminalView
    // and call cx.notify() BEFORE any deferred-capture await.
    let open_fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_tab_ai_harness_terminal_from_request(")
        .expect("open_tab_ai_harness_terminal_from_request must exist");
    let open_fn_body = &TAB_AI_MODE_SOURCE[open_fn_start..];
    let next_fn = open_fn_body[1..]
        .find("\nfn ")
        .or_else(|| open_fn_body[1..].find("\n    fn "))
        .unwrap_or(open_fn_body.len());
    let open_fn_body = &open_fn_body[..next_fn];

    let view_switch_pos = open_fn_body
        .find("AppView::QuickTerminalView")
        .expect("must switch to QuickTerminalView");
    let notify_pos = open_fn_body
        .find("cx.notify()")
        .expect("must call cx.notify()");

    // The capture_rx.recv().await must come AFTER the view switch + notify
    let capture_await_pos = open_fn_body
        .find("capture_rx.recv().await")
        .expect("must await capture_rx");

    assert!(
        view_switch_pos < capture_await_pos,
        "view switch must happen before deferred capture await"
    );
    assert!(
        notify_pos < capture_await_pos,
        "cx.notify() must happen before deferred capture await"
    );
}

#[test]
fn harness_terminal_open_path_does_not_call_capture_context_snapshot_inline() {
    // open_tab_ai_harness_terminal_from_request must NOT call
    // capture_context_snapshot directly — that now happens in the
    // deferred capture task (spawn_tab_ai_pre_switch_capture).
    let open_fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_tab_ai_harness_terminal_from_request(")
        .expect("open_tab_ai_harness_terminal_from_request must exist");
    let open_fn_body = &TAB_AI_MODE_SOURCE[open_fn_start..];
    // Find the end of this function (next fn at the same indentation)
    let next_fn = open_fn_body[1..]
        .find("\nfn ")
        .or_else(|| open_fn_body[1..].find("\n    fn "))
        .unwrap_or(open_fn_body.len());
    let open_fn_body = &open_fn_body[..next_fn];

    // The function body should NOT contain a direct call to capture_context_snapshot
    // because it now receives capture results via capture_rx channel.
    // However, capture_context_snapshot DOES appear inside the spawned async closure
    // that receives results. We check that it doesn't appear before cx.notify().
    let notify_pos = open_fn_body
        .find("cx.notify()")
        .expect("must call cx.notify()");
    let before_notify = &open_fn_body[..notify_pos];

    assert!(
        !before_notify.contains("capture_context_snapshot("),
        "open_tab_ai_harness_terminal_from_request must not call capture_context_snapshot \
         inline before cx.notify() — capture is deferred to spawn_tab_ai_pre_switch_capture"
    );
}

#[test]
fn begin_tab_ai_harness_entry_increments_capture_generation() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("tab_ai_harness_capture_generation += 1"),
        "begin_tab_ai_harness_entry must increment the capture generation counter"
    );
}

#[test]
fn app_state_has_tab_ai_harness_capture_generation() {
    assert!(
        APP_STATE_SOURCE.contains("tab_ai_harness_capture_generation: u64"),
        "app_state.rs must have a tab_ai_harness_capture_generation field"
    );
}

#[test]
fn deferred_capture_uses_bounded_channel() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("async_channel::bounded"),
        "spawn_tab_ai_pre_switch_capture must use a bounded channel"
    );
}

#[test]
fn deferred_capture_checks_generation_before_injection() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("tab_ai_harness_capture_generation != capture_gen"),
        "deferred capture must check generation counter to drop stale results"
    );
}

// =========================================================================
// BuiltInFeature::AiChat must route to harness, not legacy AI window
// =========================================================================

#[test]
fn builtin_ai_chat_entry_reflects_agent_chat_label() {
    assert!(
        BUILTINS_SOURCE.contains("\"Agent Chat\""),
        "builtin/ai-chat entry must use the Agent Chat label, not the legacy AI Chat label"
    );
}

#[test]
fn deferred_capture_stops_ignoring_launch_request() {
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn spawn_tab_ai_pre_switch_capture(")
        .expect("spawn_tab_ai_pre_switch_capture must exist");
    let fn_body = &TAB_AI_MODE_SOURCE[fn_start..];
    let next_fn = fn_body[1..].find("\n    fn ").unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_fn];

    assert!(
        !fn_body.contains("_request: &TabAiLaunchRequest"),
        "deferred capture must stop ignoring the launch request"
    );
    assert!(
        fn_body.contains("request.capture_kind"),
        "deferred capture must branch on request.capture_kind"
    );
}

#[test]
fn explicit_ai_capture_commands_use_capture_kind_plumbing() {
    assert!(
        BUILTIN_EXECUTION_SOURCE.contains("TabAiCaptureKind::FullScreen"),
        "SendScreenToAi must request FullScreen capture"
    );
    assert!(
        BUILTIN_EXECUTION_SOURCE.contains("TabAiCaptureKind::FocusedWindow"),
        "SendFocusedWindowToAi must request FocusedWindow capture"
    );
    assert!(
        BUILTIN_EXECUTION_SOURCE.contains("TabAiCaptureKind::SelectedText"),
        "SendSelectedTextToAi must request SelectedText capture"
    );
    assert!(
        BUILTIN_EXECUTION_SOURCE.contains("TabAiCaptureKind::BrowserTab"),
        "SendBrowserTabToAi must request BrowserTab capture"
    );
}

#[test]
fn capture_kind_enum_is_exported_from_ai_module() {
    assert!(
        AI_MOD_SOURCE.contains("TabAiCaptureKind"),
        "TabAiCaptureKind must be re-exported from the ai module"
    );
}

#[test]
fn tab_ai_harness_tracks_apply_back_route_state() {
    assert!(
        APP_STATE_SOURCE.contains("tab_ai_harness_apply_back_route"),
        "ScriptListApp must persist apply-back routing state for the active harness session"
    );
}

#[test]
fn quick_terminal_cmd_enter_routes_to_apply_back() {
    let dispatcher = source_block_after(
        TAB_AI_MODE_SOURCE,
        "pub(crate) fn dispatch_quick_terminal_cmd_enter",
        900,
    );
    assert!(
        dispatcher.contains("if self.quick_terminal_can_apply_back()")
            && dispatcher.contains("self.apply_tab_ai_result_from_terminal(entity, cx);")
            && dispatcher.contains("self.open_agent_chat_with_quick_terminal_output(entity, cx)"),
        "QuickTerminalView must route Cmd+Enter into apply-back only when Apply is available"
    );
}

#[test]
fn tab_ai_apply_back_uses_running_command_prompt_reinjection() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("self.try_set_prompt_input(text.clone(), cx)"),
        "RunningCommand apply-back must reuse try_set_prompt_input instead of frontmost-app paste"
    );
}

#[test]
fn tab_ai_frontmost_apply_back_hides_before_paste() {
    let hide_pos = TAB_AI_MODE_SOURCE
        .find("crate::platform::defer_hide_main_window(cx)")
        .expect("apply-back must defer-hide the main window");

    let replace_pos = TAB_AI_MODE_SOURCE
        .find("selected_text::set_selected_text(&text_for_apply)")
        .expect("apply-back must support selected-text replacement");

    let paste_pos = TAB_AI_MODE_SOURCE
        .find(".paste_text(&text_for_apply)")
        .expect("apply-back must support frontmost-app paste");

    assert!(
        hide_pos < replace_pos,
        "main window must hide before set_selected_text fires"
    );
    assert!(
        hide_pos < paste_pos,
        "main window must hide before TextInjector::paste_text fires"
    );
}

#[test]
fn tab_ai_apply_back_route_cleared_on_harness_close() {
    // close_tab_ai_harness_terminal must clear the apply-back route to prevent
    // stale routing after the harness session ends.
    let close_fn_start = TAB_AI_MODE_SOURCE
        .find("fn close_tab_ai_harness_terminal(")
        .expect("close_tab_ai_harness_terminal must exist");
    let close_fn_body = &TAB_AI_MODE_SOURCE[close_fn_start..];
    let next_fn = close_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(close_fn_body.len());
    let close_fn_body = &close_fn_body[..next_fn];

    assert!(
        close_fn_body.contains("tab_ai_harness_apply_back_route"),
        "close_tab_ai_harness_terminal must clear the apply-back route"
    );
}

#[test]
fn tab_ai_apply_back_hint_strip_visible_in_quick_terminal() {
    // The hint strip must advertise the ⌘⏎ Apply action so users know about it.
    assert!(
        TERM_RENDER_SOURCE.contains("Apply") && TERM_RENDER_SOURCE.contains("Close"),
        "QuickTerminalView hint strip must show both Apply and Close actions"
    );
}

// =========================================================================
// ScriptListItem apply-back contract
// =========================================================================

#[test]
fn script_list_item_apply_back_no_longer_shows_stub_toast() {
    // The ScriptListItem branch must NOT contain the old "not wired yet" stub.
    let apply_fn_start = TAB_AI_MODE_SOURCE
        .find("fn apply_tab_ai_result_from_clipboard(")
        .expect("apply_tab_ai_result_from_clipboard must exist");
    let apply_fn_body = &TAB_AI_MODE_SOURCE[apply_fn_start..];
    let next_impl = apply_fn_body[1..]
        .find("\nimpl ")
        .unwrap_or(apply_fn_body.len());
    let apply_fn_body = &apply_fn_body[..next_impl];

    assert!(
        !apply_fn_body.contains("not wired yet"),
        "ScriptListItem apply-back must no longer show the placeholder toast"
    );
}

#[test]
fn script_list_item_apply_back_saves_generated_script() {
    // The ScriptListItem branch must save a generated script via the existing
    // save_generated_script_from_response pipeline.
    let apply_fn_start = TAB_AI_MODE_SOURCE
        .find("fn apply_tab_ai_result_text(")
        .expect("apply_tab_ai_result_text must exist");
    let apply_fn_body = &TAB_AI_MODE_SOURCE[apply_fn_start..];

    // Find the ScriptListItem match arm.
    let arm_start = apply_fn_body
        .find("crate::ai::TabAiSourceType::ScriptListItem =>")
        .expect("ScriptListItem match arm must exist in apply-back");
    let arm_body = &apply_fn_body[arm_start..];

    assert!(
        arm_body.contains("save_generated_script_from_response"),
        "ScriptListItem apply-back must route through save_generated_script_from_response"
    );
}

#[test]
fn script_list_item_apply_back_executes_saved_script() {
    // After saving the script, the ScriptListItem branch must execute it.
    let apply_fn_start = TAB_AI_MODE_SOURCE
        .find("fn apply_tab_ai_result_text(")
        .expect("apply_tab_ai_result_text must exist");
    let apply_fn_body = &TAB_AI_MODE_SOURCE[apply_fn_start..];

    let arm_start = apply_fn_body
        .find("crate::ai::TabAiSourceType::ScriptListItem =>")
        .expect("ScriptListItem match arm must exist");
    let arm_body = &apply_fn_body[arm_start..];

    assert!(
        arm_body.contains("execute_script_by_path"),
        "ScriptListItem apply-back must execute the saved script"
    );
}

#[test]
fn script_list_item_apply_back_closes_harness_first() {
    // The harness must be closed before saving/executing the script.
    let apply_fn_start = TAB_AI_MODE_SOURCE
        .find("fn apply_tab_ai_result_text(")
        .expect("apply_tab_ai_result_text must exist");
    let apply_fn_body = &TAB_AI_MODE_SOURCE[apply_fn_start..];

    let arm_start = apply_fn_body
        .find("crate::ai::TabAiSourceType::ScriptListItem =>")
        .expect("ScriptListItem match arm must exist");
    let arm_body = &apply_fn_body[arm_start..];

    let close_pos = arm_body
        .find("close_tab_ai_harness_terminal")
        .expect("ScriptListItem apply-back must close the harness");
    let save_pos = arm_body
        .find("save_generated_script_from_response")
        .expect("ScriptListItem apply-back must save a script");

    assert!(
        close_pos < save_pos,
        "Harness must close before saving the generated script"
    );
}

#[test]
fn script_list_item_apply_back_uses_focused_target_label() {
    // The focused target label should be used as the prompt for slug derivation,
    // not a hardcoded string.
    let apply_fn_start = TAB_AI_MODE_SOURCE
        .find("fn apply_tab_ai_result_text(")
        .expect("apply_tab_ai_result_text must exist");
    let apply_fn_body = &TAB_AI_MODE_SOURCE[apply_fn_start..];

    let arm_start = apply_fn_body
        .find("crate::ai::TabAiSourceType::ScriptListItem =>")
        .expect("ScriptListItem match arm must exist");
    let arm_body = &apply_fn_body[arm_start..];

    assert!(
        arm_body.contains("route")
            && arm_body.contains("focused_target")
            && arm_body.contains(".map(|t| t.label.clone())"),
        "ScriptListItem apply-back must use the focused target's label for the prompt"
    );
}

// =========================================================================
// ClipboardEntry apply-back contract
// =========================================================================

#[test]
fn clipboard_entry_apply_back_closes_harness_first() {
    let apply_fn_start = TAB_AI_MODE_SOURCE
        .find("fn apply_tab_ai_result_text(")
        .expect("apply_tab_ai_result_text must exist");
    let apply_fn_body = &TAB_AI_MODE_SOURCE[apply_fn_start..];

    let arm_start = apply_fn_body
        .find("crate::ai::TabAiSourceType::ClipboardEntry =>")
        .expect("ClipboardEntry match arm must exist in apply-back");
    let arm_body = &apply_fn_body[arm_start..];

    let close_pos = arm_body
        .find("close_tab_ai_harness_terminal")
        .expect("ClipboardEntry apply-back must close the harness");
    let clipboard_pos = arm_body
        .find("write_tab_ai_apply_back_clipboard_text")
        .expect("ClipboardEntry apply-back must write to clipboard");

    assert!(
        close_pos < clipboard_pos,
        "Harness must close before writing clipboard result"
    );
}

#[test]
fn clipboard_entry_apply_back_writes_to_clipboard() {
    let apply_fn_start = TAB_AI_MODE_SOURCE
        .find("fn apply_tab_ai_result_text(")
        .expect("apply_tab_ai_result_text must exist");
    let apply_fn_body = &TAB_AI_MODE_SOURCE[apply_fn_start..];

    let arm_start = apply_fn_body
        .find("crate::ai::TabAiSourceType::ClipboardEntry =>")
        .expect("ClipboardEntry match arm must exist");
    let arm_body = &apply_fn_body[arm_start..];

    assert!(
        arm_body.contains("write_tab_ai_apply_back_clipboard_text"),
        "ClipboardEntry apply-back must use write_tab_ai_apply_back_clipboard_text"
    );
}

// =========================================================================
// Desktop / DesktopSelection apply-back contract
// =========================================================================

#[test]
fn desktop_selection_apply_back_uses_set_selected_text() {
    let apply_fn_start = TAB_AI_MODE_SOURCE
        .find("fn apply_tab_ai_result_text(")
        .expect("apply_tab_ai_result_text must exist");
    let apply_fn_body = &TAB_AI_MODE_SOURCE[apply_fn_start..];

    // The DesktopSelection arm should call set_selected_text
    assert!(
        apply_fn_body.contains("selected_text::set_selected_text(&text_for_apply)"),
        "DesktopSelection apply-back must replace via set_selected_text"
    );
}

#[test]
fn generic_desktop_apply_back_uses_paste_text() {
    let apply_fn_start = TAB_AI_MODE_SOURCE
        .find("fn apply_tab_ai_result_text(")
        .expect("apply_tab_ai_result_text must exist");
    let apply_fn_body = &TAB_AI_MODE_SOURCE[apply_fn_start..];

    // The Desktop arm should use TextInjector::paste_text
    assert!(
        apply_fn_body.contains(".paste_text(&text_for_apply)"),
        "Desktop (generic) apply-back must paste via TextInjector"
    );
}

#[test]
fn apply_back_match_covers_all_five_source_types() {
    let apply_fn_start = TAB_AI_MODE_SOURCE
        .find("fn apply_tab_ai_result_text(")
        .expect("apply_tab_ai_result_text must exist");
    let apply_fn_body = &TAB_AI_MODE_SOURCE[apply_fn_start..];

    for arm in &[
        "crate::ai::TabAiSourceType::RunningCommand =>",
        "crate::ai::TabAiSourceType::ClipboardEntry =>",
        "crate::ai::TabAiSourceType::ScriptListItem =>",
        "crate::ai::TabAiSourceType::DesktopSelection\n            | crate::ai::TabAiSourceType::Desktop =>",
    ] {
        assert!(
            apply_fn_body.contains(arm),
            "apply_tab_ai_result_from_clipboard must handle {arm}"
        );
    }
}

// =========================================================================
// Script List context shaping: explicit branch in target resolution
// =========================================================================

#[test]
fn resolve_targets_has_explicit_script_list_branch() {
    // resolve_tab_ai_surface_targets_for_view must have an explicit
    // AppView::ScriptList arm that uses script-native metadata, not
    // the generic element-based fallback.
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn resolve_tab_ai_surface_targets_for_view(")
        .expect("resolve_tab_ai_surface_targets_for_view must exist");
    let fn_body = &TAB_AI_MODE_SOURCE[fn_start..];
    let next_fn = fn_body[1..].find("\n    fn ").unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_fn];

    assert!(
        fn_body.contains("AppView::ScriptList =>"),
        "resolve_tab_ai_surface_targets_for_view must have an explicit ScriptList arm"
    );
}

#[test]
fn script_list_branch_delegates_to_search_result_helper() {
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn resolve_tab_ai_surface_targets_for_view(")
        .expect("resolve_tab_ai_surface_targets_for_view must exist");
    let fn_body = &TAB_AI_MODE_SOURCE[fn_start..];
    let next_fn = fn_body[1..].find("\n    fn ").unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_fn];

    assert!(
        fn_body.contains("tab_ai_target_from_search_result"),
        "ScriptList branch must use tab_ai_target_from_search_result for rich metadata"
    );
}

#[test]
fn tab_ai_target_from_search_result_helper_exists() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("fn tab_ai_target_from_search_result("),
        "tab_ai_target_from_search_result helper must exist in agent_handoff.rs"
    );
}

#[test]
fn search_result_helper_produces_script_list_source() {
    // The helper must tag all targets with source "ScriptList" so that
    // detect_tab_ai_source_type can classify them correctly.
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn tab_ai_target_from_search_result(")
        .expect("tab_ai_target_from_search_result must exist");
    let fn_body = &TAB_AI_MODE_SOURCE[fn_start..];
    let next_fn = fn_body[1..].find("\n    fn ").unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_fn];

    assert!(
        fn_body.contains("\"ScriptList\""),
        "tab_ai_target_from_search_result must set source to \"ScriptList\""
    );
}

#[test]
fn search_result_helper_maps_all_result_kinds() {
    // Every SearchResult variant must have a corresponding kind string.
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn tab_ai_target_from_search_result(")
        .expect("tab_ai_target_from_search_result must exist");
    let fn_body = &TAB_AI_MODE_SOURCE[fn_start..];
    let next_fn = fn_body[1..].find("\n    fn ").unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_fn];

    for kind in &[
        "\"script\"",
        "\"scriptlet\"",
        "\"builtin\"",
        "\"app\"",
        "\"window\"",
        "\"agent\"",
        "\"fallback\"",
    ] {
        assert!(
            fn_body.contains(kind),
            "tab_ai_target_from_search_result must map kind {kind}"
        );
    }
}

// =========================================================================
// Harness close lifecycle: full contract assertions
// =========================================================================

#[test]
fn close_tab_ai_harness_terminal_clears_session_and_schedules_fresh_prewarm() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("self.tab_ai_harness_capture_generation += 1;"),
        "close must invalidate in-flight deferred capture results",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("self.tab_ai_harness_apply_back_route = None;"),
        "close must clear apply-back routing state",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("self.terminate_tab_ai_harness_session(cx);"),
        "close must clear app state through terminate_tab_ai_harness_session()",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("term.terminate_session().map_err(|e| e.to_string())"),
        "close must terminate the PTY session",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains(
            "self.schedule_tab_ai_harness_prewarm(std::time::Duration::from_millis(250), cx);"
        ),
        "close must schedule a fresh harness prewarm after teardown",
    );
}

#[test]
fn quick_terminal_cmd_w_routes_to_harness_close() {
    assert!(
        TERM_RENDER_SOURCE.contains("key.eq_ignore_ascii_case(\"w\")"),
        "QuickTerminalView must reserve Cmd+W as the wrapper close gesture",
    );
    assert!(
        TERM_RENDER_SOURCE.contains("this.close_quick_terminal_main_window_state_first(cx);"),
        "Cmd+W must call the state-first main-window close path",
    );
}

#[test]
fn quick_terminal_state_first_close_hides_before_script_list_can_render() {
    let helper = source_block_after(
        TAB_AI_MODE_SOURCE,
        "fn close_quick_terminal_main_window_state_first(",
        900,
    );
    assert!(
        helper.contains("TabAiHarnessCloseDisposition::CloseMainWindowStateFirst"),
        "Quick Terminal Cmd+W helper must select the state-first close disposition",
    );

    let impl_body = source_block_after(
        TAB_AI_MODE_SOURCE,
        "fn close_tab_ai_harness_terminal_impl(",
        5200,
    );
    let state_first_branch = impl_body
        .find("TabAiHarnessCloseDisposition::CloseMainWindowStateFirst")
        .expect("state-first close branch must exist");
    let close_reset = impl_body[state_first_branch..]
        .find("self.close_and_reset_window(cx);")
        .map(|offset| state_first_branch + offset)
        .expect("state-first branch must delegate to close_and_reset_window");
    let visible_restore = impl_body[state_first_branch..]
        .find("self.restore_current_view_with_focus(return_view, return_focus_target)")
        .map(|offset| state_first_branch + offset);
    assert!(
        visible_restore.map_or(true, |restore| close_reset < restore),
        "state-first close must not restore the return view before close_and_reset_window hides the main window",
    );
}

#[test]
fn search_result_helper_includes_script_metadata() {
    // Script-type results must include path and description in metadata.
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn tab_ai_target_from_search_result(")
        .expect("tab_ai_target_from_search_result must exist");
    let fn_body = &TAB_AI_MODE_SOURCE[fn_start..];
    let next_fn = fn_body[1..].find("\n    fn ").unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_fn];

    // Script metadata must include path and description for AI context.
    assert!(
        fn_body.contains("\"path\"") && fn_body.contains("\"description\""),
        "Script metadata must include path and description"
    );
    // Script metadata must include shortcut and alias for context.
    assert!(
        fn_body.contains("\"shortcut\"") && fn_body.contains("\"alias\""),
        "Script metadata must include shortcut and alias"
    );
}

#[test]
fn script_generation_compat_shims_documented_as_harness_only() {
    assert!(
        PROMPT_AI_SOURCE.contains("Compatibility shim \u{2014} routes script generation requests"),
        "prompt_ai.rs must document the harness-only compatibility shim",
    );
}

#[test]
fn script_generation_compat_shims_do_not_reconstruct_legacy_surface() {
    // The old script-generation ChatPrompt surface used "script-generation"
    // as its ID. The compat shims must not recreate that surface.
    assert!(
        !PROMPT_AI_SOURCE.contains("\"script-generation\""),
        "prompt_ai.rs must not recreate the old script-generation ChatPrompt surface",
    );
}

// =========================================================================
// "Send to AI" fallback: priority 0, harness-native, inline execution
// =========================================================================

const FALLBACK_BUILTINS_SOURCE: &str = include_str!("../src/fallbacks/builtins.rs");
const SELECTION_FALLBACK_SOURCE: &str = include_str!("../src/app_impl/selection_fallback.rs");

#[test]
fn send_to_ai_fallback_label_is_constant() {
    assert!(
        FALLBACK_BUILTINS_SOURCE
            .contains("pub const SEND_TO_AI_FALLBACK_LABEL: &str = \"Auto Submit\""),
        "the top fallback label must be the constant 'Auto Submit'",
    );
}

#[test]
fn send_to_ai_fallback_is_harness_native() {
    assert!(
        FALLBACK_BUILTINS_SOURCE.contains("action: FallbackAction::SendToAiHarness"),
        "the top fallback must use FallbackAction::SendToAiHarness",
    );
}

#[test]
fn send_to_ai_fallback_is_priority_zero() {
    assert!(
        FALLBACK_BUILTINS_SOURCE.contains("priority: 0"),
        "the harness fallback must be priority 0 (first in no-results list)",
    );
}

#[test]
fn selection_fallback_recognizes_harness_result() {
    assert!(
        SELECTION_FALLBACK_SOURCE.contains("FallbackResult::SendToAiHarness { query }"),
        "selection fallback execution must recognize the SendToAiHarness result variant",
    );
}

#[test]
fn selection_fallback_routes_to_agent_chat() {
    assert!(
        SELECTION_FALLBACK_SOURCE.contains("self.open_tab_ai_agent_chat_with_entry_intent("),
        "Send to AI fallback must route to Agent Chat chat",
    );
}

// =========================================================================
// Full-screen capture helper: contract and metadata
// =========================================================================

#[test]
fn full_screen_capture_helper_exists_and_uses_screen_api() {
    assert!(
        SCREENSHOT_FILES_SOURCE.contains("pub fn capture_tab_ai_screen_screenshot_file()"),
        "full-screen screenshot helper must exist as a public function",
    );
    assert!(
        SCREENSHOT_FILES_SOURCE.contains("capture_screen_screenshot()"),
        "full-screen helper must call the platform full-screen screenshot API",
    );
    assert!(
        SCREENSHOT_FILES_SOURCE.contains("title: \"Full Screen\".to_string()"),
        "full-screen helper must label the artifact as 'Full Screen'",
    );
}

#[test]
fn full_screen_capture_helper_preserves_screenshot_file_contract() {
    assert!(
        SCREENSHOT_FILES_SOURCE.contains("cleanup_old_tab_ai_screenshot_files"),
        "full-screen helper must clean up old screenshot temp files",
    );
    assert!(
        SCREENSHOT_FILES_SOURCE.contains("TAB_AI_SCREENSHOT_MAX_KEEP"),
        "full-screen helper must use the shared screenshot retention limit",
    );
    assert!(
        SCREENSHOT_FILES_SOURCE.contains("used_fallback: false"),
        "full-screen helper must set used_fallback to false in screenshot metadata",
    );
}

// =========================================================================
// Builtin registry: legacy AI window entries removed, supported paths kept
// =========================================================================

#[test]
fn legacy_ai_window_entries_stay_removed_while_manual_paths_stay_present() {
    let fn_start = BUILTINS_SOURCE
        .find("pub fn get_builtin_entries(")
        .expect("get_builtin_entries must exist");
    let fn_body = &BUILTINS_SOURCE[fn_start..];
    let fn_end = fn_body.find("\n#[cfg(test)]").unwrap_or(fn_body.len());
    let registration_section = &fn_body[..fn_end];

    // Legacy window-style AI builtins must NOT be registered.
    // Match quoted string literals to avoid false positives from comments.
    for legacy_id in [
        "builtin/open-ai-chat",
        "builtin/mini-ai-chat",
        "builtin/new-conversation",
        "builtin/clear-conversation",
        "builtin/send-screen-area-to-ai",
    ] {
        let quoted = format!("\"{}\"", legacy_id);
        assert!(
            !registration_section.contains(&quoted),
            "{legacy_id} must not be registered in the main builtin list",
        );
    }

    // Harness-first AI entries and manual creation paths must remain registered.
    for kept_id in [
        "builtin/generate-script-with-ai",
        "builtin/generate-script-from-current-app",
        "builtin/send-screen-to-ai",
        "builtin/send-selected-text-to-ai",
        "builtin/send-browser-tab-to-ai",
        "builtin/new-script",
        "builtin/new-extension",
    ] {
        let quoted = format!("\"{}\"", kept_id);
        assert!(
            registration_section.contains(&quoted),
            "{kept_id} should remain registered",
        );
    }
}

// ---------------------------------------------------------------------------
// Live-session quick submit: structured submission regression tests
// ---------------------------------------------------------------------------

#[test]
fn live_quick_submit_uses_structured_submission_helper() {
    let src = std::fs::read_to_string("src/app_impl/agent_handoff/mod.rs")
        .expect("read agent_handoff.rs");

    let fn_start = src
        .find("pub(crate) fn submit_to_current_or_new_tab_ai_harness_from_text(")
        .expect("submit_to_current_or_new_tab_ai_harness_from_text must exist");
    let fn_src = &src[fn_start..fn_start + 2500.min(src.len() - fn_start)];

    assert!(
        fn_src.contains("self.submit_live_tab_ai_harness_from_plan("),
        "live harness quick submit must route through the structured submission helper"
    );
    assert!(
        !fn_src.contains("format!(\"{}\\n\", plan.synthesized_intent)"),
        "live harness quick submit must not inject raw intent-only text"
    );
}

#[test]
fn live_quick_submit_helper_builds_full_harness_submission() {
    let src = std::fs::read_to_string("src/app_impl/agent_handoff/mod.rs")
        .expect("read agent_handoff.rs");

    let helper_start = src
        .find("fn submit_live_tab_ai_harness_from_plan(")
        .expect("submit_live_tab_ai_harness_from_plan must exist");
    let helper_src = &src[helper_start..helper_start + 6000.min(src.len() - helper_start)];

    assert!(
        helper_src.contains("build_tab_ai_harness_submission"),
        "live quick submit must build a full structured harness submission"
    );
    assert!(
        helper_src.contains("TabAiHarnessSubmissionMode::Submit"),
        "live quick submit must submit immediately"
    );
    assert!(
        helper_src.contains("request.quick_submit_plan.as_ref()"),
        "live quick submit must pass quick submit plan to submission builder"
    );
}

// =========================================================================
// ~ mini entry: file search presentation and query normalization
// =========================================================================

const FILTER_INPUT_CHANGE_SOURCE: &str = include_str!("../src/app_impl/filter_input_change.rs");
const FILTER_INPUT_CORE_SOURCE: &str = include_str!("../src/app_impl/filter_input_core.rs");
const FILE_SEARCH_SOURCE: &str = include_str!("../src/render_builtins/file_search.rs");

#[test]
fn tilde_trigger_enters_file_search_as_mini() {
    // When the user types `~` in ScriptList, filter_input_change must hand off
    // to mini file search with FileSearchPresentation::Mini.
    assert!(
        FILTER_INPUT_CHANGE_SOURCE.contains("should_enter_file_search_from_script_list"),
        "filter_input_change must call should_enter_file_search_from_script_list for ~ trigger"
    );
    assert!(
        FILTER_INPUT_CHANGE_SOURCE.contains("FileSearchPresentation::Mini"),
        "filter_input_change must open file search as Mini presentation"
    );
}

#[test]
fn tilde_trigger_predicate_matches_tilde_and_tilde_slash_prefix() {
    // should_enter_file_search_from_script_list must match `~` and `~/...`
    assert!(
        FILTER_INPUT_CORE_SOURCE.contains(r#"new_text == "~" || new_text.starts_with("~/")"#),
        "should_enter_file_search_from_script_list must match ~ and ~/... patterns"
    );
}

#[test]
fn file_search_cmd_enter_routes_through_selection_or_query() {
    // ⌘↵ in file search must call open_file_search_selection_or_query_in_tab_ai,
    // which tries the selected row first, then falls back to query-level intent.
    assert!(
        FILE_SEARCH_SOURCE.contains("open_file_search_selection_or_query_in_tab_ai"),
        "file_search.rs Enter handler must call the fallback-capable AI opener"
    );
}

#[test]
fn selection_or_query_tries_selection_first() {
    // open_file_search_selection_or_query_in_tab_ai must attempt the selection
    // path before falling back to query-level intent.
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_file_search_selection_or_query_in_tab_ai(")
        .expect("open_file_search_selection_or_query_in_tab_ai must exist");
    let fn_body =
        &TAB_AI_MODE_SOURCE[fn_start..fn_start + 800.min(TAB_AI_MODE_SOURCE.len() - fn_start)];

    assert!(
        fn_body.contains("open_file_search_selection_in_tab_ai"),
        "must try selection-based AI first"
    );
    // The selection attempt must come before the query fallback
    let sel_pos = fn_body
        .find("open_file_search_selection_in_tab_ai")
        .unwrap();
    let query_pos = fn_body
        .find("build_file_search_ai_query_intent")
        .expect("must have query fallback");
    assert!(
        sel_pos < query_pos,
        "selection AI must be attempted before query fallback"
    );
}

#[test]
fn query_fallback_returns_none_for_empty_state() {
    // build_file_search_ai_query_intent must return None when both query is
    // empty and there are no visible results, so ⌘↵ is a no-op only in the
    // truly empty state.
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn build_file_search_ai_query_intent(")
        .expect("build_file_search_ai_query_intent must exist");
    let fn_body =
        &TAB_AI_MODE_SOURCE[fn_start..fn_start + 400.min(TAB_AI_MODE_SOURCE.len() - fn_start)];

    assert!(
        fn_body.contains("query.is_empty() && self.file_search_display_indices.is_empty()"),
        "query intent must return None only when both query and results are empty"
    );
    assert!(
        fn_body.contains("return None"),
        "must explicitly return None for the empty state"
    );
}

#[test]
fn selection_ai_uses_quick_submit_with_file_search_source() {
    // open_file_search_selection_in_tab_ai must build a TabAiQuickSubmitPlan
    // with source = FileSearch so harness logging has provenance.
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_file_search_selection_in_tab_ai(")
        .expect("open_file_search_selection_in_tab_ai must exist");
    let fn_body =
        &TAB_AI_MODE_SOURCE[fn_start..fn_start + 1500.min(TAB_AI_MODE_SOURCE.len() - fn_start)];

    assert!(
        fn_body.contains("TabAiQuickSubmitSource::FileSearch"),
        "selection AI must use FileSearch as the quick submit source"
    );
    assert!(
        fn_body.contains("TabAiQuickSubmitKind::FileDrop"),
        "selection AI must use FileDrop as the quick submit kind"
    );
    assert!(
        fn_body.contains("open_tab_ai_chat_with_quick_submit_plan"),
        "selection AI must route through the quick submit plan path"
    );
}

#[test]
fn query_fallback_uses_entry_intent_not_quick_submit() {
    // When no row is selected, query-level AI uses open_tab_ai_chat_with_entry_intent
    // (not quick submit), because there is no specific file to submit.
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_file_search_selection_or_query_in_tab_ai(")
        .expect("open_file_search_selection_or_query_in_tab_ai must exist");
    let fn_body =
        &TAB_AI_MODE_SOURCE[fn_start..fn_start + 800.min(TAB_AI_MODE_SOURCE.len() - fn_start)];

    assert!(
        fn_body.contains("open_tab_ai_chat_with_entry_intent"),
        "query fallback must use entry intent (not quick submit)"
    );
}

#[test]
fn file_search_ai_intent_includes_presentation_label() {
    // Both build_file_search_ai_entry_intent and build_file_search_ai_query_intent
    // must embed the current presentation (mini vs full) so the AI model knows
    // which surface the user is on.
    let entry_fn_start = TAB_AI_MODE_SOURCE
        .find("fn build_file_search_ai_entry_intent(")
        .expect("build_file_search_ai_entry_intent must exist");
    let entry_fn_body = &TAB_AI_MODE_SOURCE
        [entry_fn_start..entry_fn_start + 2000.min(TAB_AI_MODE_SOURCE.len() - entry_fn_start)];
    assert!(
        entry_fn_body.contains("File-search presentation:"),
        "entry intent must include presentation label"
    );

    let query_fn_start = TAB_AI_MODE_SOURCE
        .find("fn build_file_search_ai_query_intent(")
        .expect("build_file_search_ai_query_intent must exist");
    let query_fn_body = &TAB_AI_MODE_SOURCE
        [query_fn_start..query_fn_start + 2000.min(TAB_AI_MODE_SOURCE.len() - query_fn_start)];
    assert!(
        query_fn_body.contains("File-search presentation:"),
        "query intent must include presentation label"
    );
}

#[test]
fn file_search_cmd_enter_passes_shift_for_plan_mode() {
    // The ⌘↵ handler must check shift state and pass it as plan_mode,
    // distinguishing ⌘↵ (explain) from ⌘⇧↵ (plan).
    let handler_area = FILE_SEARCH_SOURCE
        .find("open_file_search_selection_or_query_in_tab_ai")
        .expect("AI opener call must exist in file_search.rs");
    let nearby = &FILE_SEARCH_SOURCE[handler_area.saturating_sub(300)
        ..handler_area + 200.min(FILE_SEARCH_SOURCE.len() - handler_area)];

    assert!(
        nearby.contains("has_shift") || nearby.contains("modifiers.shift"),
        "⌘↵ handler must distinguish shift for plan_mode"
    );
}

// =========================================================================
// Agent Chat routing: Tab now opens AgentChatView instead of QuickTerminalView
// =========================================================================

const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/view.rs");
const AGENT_CHAT_THREAD_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/thread.rs");
const AGENT_CHAT_MOD_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/mod.rs");
const AGENT_CHAT_WINDOW_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/chat_window.rs");

#[test]
fn detached_agent_chat_reuse_preserves_entry_intent() {
    let open_fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_tab_ai_agent_chat_with_options(")
        .expect("open_tab_ai_agent_chat_with_options must exist");
    let open_fn_body = &TAB_AI_MODE_SOURCE[open_fn_start..];
    let next_fn = open_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(open_fn_body.len());
    let open_fn_body = &open_fn_body[..next_fn];

    let normalized_idx = open_fn_body
        .find("let normalized_entry_intent = entry_intent")
        .expect("entry intent must be normalized for detached Agent Chat reuse");
    let detached_idx = open_fn_body
        .find("if crate::ai::agent_chat::ui::chat_window::is_chat_window_open()")
        .expect("detached Agent Chat reuse branch must exist");

    assert!(
        normalized_idx < detached_idx,
        "entry intent must be normalized before detached Agent Chat reuse is checked"
    );
    assert!(
        open_fn_body.contains("submit_reused_entry_intent_in_detached_chat"),
        "detached Agent Chat reuse must route fresh entry intent into the live detached view"
    );
    assert!(
        AGENT_CHAT_WINDOW_SOURCE
            .contains("event = \"tab_ai_reused_detached_window_with_entry_intent\""),
        "detached Agent Chat reuse helper must emit a dedicated structured log event"
    );
}

#[test]
fn detached_agent_chat_reuse_with_host_context_submits_atomically() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn submit_reused_entry_intent_with_host_context("),
        "AgentChatView must expose a combined reuse helper for host context plus fresh intent"
    );
    let fn_start = AGENT_CHAT_VIEW_SOURCE
        .find("fn submit_reused_entry_intent_with_host_context(")
        .expect("combined Agent Chat reuse helper must exist");
    let fn_body = &AGENT_CHAT_VIEW_SOURCE[fn_start..];
    let next_fn = fn_body[1..].find("\n    fn ").unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_fn];

    assert!(
        fn_body.contains("thread.replace_pending_context_parts(staged_parts, source, cx);"),
        "combined Agent Chat reuse helper must replace stale pending context"
    );
    assert!(
        fn_body.contains("thread.submit_input(cx)"),
        "combined Agent Chat reuse helper must submit after staging fresh context and intent"
    );
    assert!(
        AGENT_CHAT_WINDOW_SOURCE
            .contains("submit_reused_entry_intent_with_host_context_in_detached_chat"),
        "detached chat window must expose a host-context-aware reuse wrapper"
    );
    assert!(
        AGENT_CHAT_WINDOW_SOURCE.contains("chat.submit_reused_entry_intent_with_host_context("),
        "detached chat reuse must delegate into the shared AgentChatView helper"
    );
}

#[test]
fn startup_guards_against_double_agent_chat_open() {
    // Both startup.rs variants must check for AgentChatView to prevent
    // stacking a second Agent Chat session on Tab press.
    assert!(
        TAB_SOURCE.contains("AppView::AgentChatView"),
        "startup.rs must check for existing AgentChatView"
    );
    assert!(
        TAB_SOURCE.contains("handle_tab_key"),
        "startup.rs must delegate Tab to AgentChatView.handle_tab_key"
    );

    assert!(
        TAB_NEW_SOURCE.contains("AppView::AgentChatView"),
        "startup_new_tab.rs must check for existing AgentChatView"
    );
    assert!(
        TAB_NEW_SOURCE.contains("handle_tab_key"),
        "startup_new_tab.rs must delegate Tab to AgentChatView.handle_tab_key"
    );
}

#[test]
fn agent_chat_view_consumes_tab_to_prevent_reentry() {
    // AgentChatView.handle_tab_key must return true to consume the key.
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn handle_tab_key"),
        "AgentChatView must implement handle_tab_key"
    );
    // The function must return true to consume Tab.
    let fn_start = AGENT_CHAT_VIEW_SOURCE
        .find("fn handle_tab_key")
        .expect("handle_tab_key must exist");
    let fn_body = &AGENT_CHAT_VIEW_SOURCE
        [fn_start..fn_start + 300.min(AGENT_CHAT_VIEW_SOURCE.len() - fn_start)];
    assert!(
        fn_body.contains("true"),
        "handle_tab_key must return true to consume Tab"
    );
}

#[test]
fn pty_path_still_exists_for_script_terminals() {
    // The PTY/harness path must still exist for script-triggered terminals.
    assert!(
        TAB_AI_MODE_SOURCE.contains("fn open_tab_ai_harness_terminal_from_request"),
        "PTY harness terminal function must still exist"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("AppView::QuickTerminalView"),
        "PTY path must still set QuickTerminalView"
    );
}

#[test]
fn agent_chat_thread_supports_staged_context_and_initial_input() {
    // AgentChatThread must support staged context blocks and initial input for auto-submit.
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("pending_context_blocks"),
        "AgentChatThread must have staged context fields"
    );
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("stage_context"),
        "AgentChatThread must expose a stage_context method"
    );
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("initial_input"),
        "AgentChatThreadInit must accept initial_input for auto-submit"
    );
}

#[test]
fn render_impl_handles_agent_chat_view() {
    assert!(
        RENDER_IMPL_SOURCE.contains("AppView::AgentChatView"),
        "render_impl must dispatch AgentChatView for rendering"
    );
}

#[test]
fn agent_chat_view_has_inline_permission_card() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("render_permission_inline_card"),
        "AgentChatView must render an inline permission approval card"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("approve_pending_permission"),
        "AgentChatView permission card must call approve_pending_permission"
    );
}

#[test]
fn agent_chat_view_has_empty_and_streaming_states() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("Ask anything\\u{2026}")
            || AGENT_CHAT_VIEW_SOURCE.contains("Ask anything…"),
        "AgentChatView must expose an empty-state placeholder"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("agent_chat-streaming-dot"),
        "AgentChatView must have a streaming indicator"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("Follow up\\u{2026}")
            || AGENT_CHAT_VIEW_SOURCE.contains("Follow up…"),
        "AgentChatView must expose a follow-up placeholder once messages exist"
    );
}

#[test]
fn agent_chat_mod_exports_required_types() {
    // The Agent Chat module must re-export the key types used by agent_handoff.
    assert!(AGENT_CHAT_MOD_SOURCE.contains("AgentChatView"));
    assert!(AGENT_CHAT_MOD_SOURCE.contains("AgentChatThread"));
    assert!(AGENT_CHAT_MOD_SOURCE.contains("AgentChatPermissionBroker"));
    assert!(!AGENT_CHAT_MOD_SOURCE.contains(concat!("AgentChat", "Runtime")));
}

// =========================================================================
// Focused-target chip routing and Ask Anything fallback
// =========================================================================

#[test]
fn tab_ai_default_path_stages_focused_target_chip() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("build_tab_ai_focused_part_for_view"),
        "default Tab path must build a focused-target context part",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("AiContextPart::FocusedTarget"),
        "default Tab path must use a FocusedTarget context part",
    );
}

#[test]
fn tab_ai_has_explicit_ask_anything_fallback() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("tab_ai_ask_anything_fallback"),
        "Tab AI routing must have an explicit Ask Anything fallback log/event",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("should_use_tab_ai_ask_anything_fallback"),
        "Tab AI routing must have a should_use_tab_ai_ask_anything_fallback helper",
    );
}

#[test]
fn tab_ai_focused_path_skips_ambient_capture() {
    assert!(
        TAB_AI_MODE_SOURCE.contains(
            "let use_ask_anything_fallback = self.should_use_tab_ai_ask_anything_fallback("
        ),
        "begin_tab_ai_harness_entry must resolve ask-anything fallback before capture dispatch",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("tab_ai_focus_chip_staged"),
        "focused-target path must log tab_ai_focus_chip_staged",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("tab_ai_ask_anything_fallback"),
        "fallback path must log tab_ai_ask_anything_fallback",
    );
}

#[test]
fn agent_chat_view_renders_pending_context_chips() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("render_pending_context_chips"),
        "AgentChatView must render pending context chips",
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("agent_chat-pending-context-chips"),
        "AgentChatView must have an element ID for the chips container",
    );
}

#[test]
fn agent_chat_thread_supports_context_parts() {
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("pending_context_parts"),
        "AgentChatThread must store typed context parts",
    );
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("fn add_context_part("),
        "AgentChatThread must have an add_context_part method",
    );
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("fn remove_context_part("),
        "AgentChatThread must have a remove_context_part method",
    );
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("fn mark_context_bootstrap_ready("),
        "AgentChatThread must have a mark_context_bootstrap_ready method",
    );
}

#[test]
fn agent_chat_thread_resolves_context_parts_on_submit() {
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("agent_chat_submit_resolved_context_parts"),
        "AgentChatThread must resolve context parts at submit time and log it",
    );
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("resolve_context_parts_with_receipt"),
        "AgentChatThread must use resolve_context_parts_with_receipt for typed parts",
    );
}

// =========================================================================
// Query text and input text as first-class focused targets
// =========================================================================

#[test]
fn file_search_query_resolves_search_query_target_when_no_row_selected() {
    // When FileSearchView has a non-empty query but no selected row,
    // resolve_tab_ai_surface_targets_for_view must produce a search_query target.
    let resolve_fn_start = TAB_AI_MODE_SOURCE
        .find("fn resolve_tab_ai_surface_targets_for_view(")
        .expect("resolve_tab_ai_surface_targets_for_view must exist");
    let resolve_fn_body = &TAB_AI_MODE_SOURCE[resolve_fn_start..];

    // Find the FileSearchView arm
    let fs_arm_pos = resolve_fn_body
        .find("AppView::FileSearchView")
        .expect("FileSearchView arm must exist in target resolver");
    let fs_arm_body = &resolve_fn_body[fs_arm_pos..];

    assert!(
        fs_arm_body.contains("tab_ai_target_from_search_query"),
        "FileSearchView must fall back to tab_ai_target_from_search_query when no row is selected",
    );
    assert!(
        fs_arm_body.contains(r#""FileSearch""#),
        "FileSearchView search_query target must use source 'FileSearch'",
    );
}

#[test]
fn script_list_filter_resolves_search_query_target_when_no_row_selected() {
    let resolve_fn_start = TAB_AI_MODE_SOURCE
        .find("fn resolve_tab_ai_surface_targets_for_view(")
        .expect("resolve_tab_ai_surface_targets_for_view must exist");
    let resolve_fn_body = &TAB_AI_MODE_SOURCE[resolve_fn_start..];

    let sl_arm_pos = resolve_fn_body
        .find("AppView::ScriptList =>")
        .expect("ScriptList arm must exist in target resolver");
    let sl_arm_body = &resolve_fn_body[sl_arm_pos..];

    assert!(
        sl_arm_body.contains("tab_ai_target_from_search_query"),
        "ScriptList must fall back to tab_ai_target_from_search_query when no row is selected",
    );
    assert!(
        sl_arm_body.contains(r#""ScriptList""#),
        "ScriptList search_query target must use source 'ScriptList'",
    );
    assert!(
        sl_arm_body.contains("visibleResultCount"),
        "ScriptList search_query metadata must include visibleResultCount",
    );
}

#[test]
fn generic_prompt_input_resolves_input_target_when_no_semantic_selection() {
    let resolve_fn_start = TAB_AI_MODE_SOURCE
        .find("fn resolve_tab_ai_surface_targets_for_view(")
        .expect("resolve_tab_ai_surface_targets_for_view must exist");
    let resolve_fn_body = &TAB_AI_MODE_SOURCE[resolve_fn_start..];

    // The wildcard arm uses `_ => {` inside resolve_tab_ai_surface_targets_for_view
    let resolve_fn_end = resolve_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(resolve_fn_body.len());
    let resolve_fn_body = &resolve_fn_body[..resolve_fn_end];

    let wildcard_pos = resolve_fn_body
        .rfind("_ => {")
        .expect("wildcard arm must exist in target resolver");
    let wildcard_body = &resolve_fn_body[wildcard_pos..];

    assert!(
        wildcard_body.contains("tab_ai_target_from_input_text"),
        "wildcard arm must fall back to tab_ai_target_from_input_text when no semantic target",
    );
    assert!(
        wildcard_body.contains("ui.input_text"),
        "wildcard arm must check ui.input_text for the input fallback",
    );
}

#[test]
fn chip_prefix_maps_search_query_to_search() {
    assert!(
        TAB_AI_MODE_SOURCE.contains(r#""search_query" => "Search""#),
        "tab_ai_chip_prefix_for_kind must map search_query to 'Search'",
    );
}

#[test]
fn chip_prefix_maps_input_to_input() {
    assert!(
        TAB_AI_MODE_SOURCE.contains(r#""input" => "Input""#),
        "tab_ai_chip_prefix_for_kind must map input to 'Input'",
    );
}

#[test]
fn search_query_target_builder_logs_resolution_event() {
    let builder_start = TAB_AI_MODE_SOURCE
        .find("fn tab_ai_target_from_search_query(")
        .expect("tab_ai_target_from_search_query must exist");
    let builder_body = &TAB_AI_MODE_SOURCE[builder_start..];
    let next_fn = builder_body[1..]
        .find("\n    fn ")
        .unwrap_or(builder_body.len());
    let builder_body = &builder_body[..next_fn];

    assert!(
        builder_body.contains(r#"event = "tab_ai_search_query_target_resolved""#),
        "search query target builder must log tab_ai_search_query_target_resolved",
    );
    assert!(
        builder_body.contains(r#"kind: "search_query""#),
        "search query target must have kind 'search_query'",
    );
}

#[test]
fn input_target_builder_logs_resolution_event() {
    let builder_start = TAB_AI_MODE_SOURCE
        .find("fn tab_ai_target_from_input_text(")
        .expect("tab_ai_target_from_input_text must exist");
    let builder_body = &TAB_AI_MODE_SOURCE[builder_start..];
    let next_fn = builder_body[1..]
        .find("\n    fn ")
        .unwrap_or(builder_body.len());
    let builder_body = &builder_body[..next_fn];

    assert!(
        builder_body.contains(r#"event = "tab_ai_input_target_resolved""#),
        "input target builder must log tab_ai_input_target_resolved",
    );
    assert!(
        builder_body.contains(r#"kind: "input""#),
        "input target must have kind 'input'",
    );
    assert!(
        builder_body.contains("inputPreview"),
        "input target metadata must include inputPreview (not the full body)",
    );
    assert!(
        builder_body.contains("inputLength"),
        "input target metadata must include inputLength",
    );
}

#[test]
fn ask_anything_fallback_only_when_no_focused_target() {
    assert!(
        TAB_AI_MODE_SOURCE.contains(
            "let use_ask_anything_fallback = self.should_use_tab_ai_ask_anything_fallback("
        ),
        "begin_tab_ai_harness_entry must call should_use_tab_ai_ask_anything_fallback",
    );
    // The fallback helper must check for absence of focused target
    let fallback_fn_start = TAB_AI_MODE_SOURCE
        .find("fn should_use_tab_ai_ask_anything_fallback(")
        .expect("should_use_tab_ai_ask_anything_fallback must exist");
    let fallback_fn_body = &TAB_AI_MODE_SOURCE[fallback_fn_start..];
    let next_fn = fallback_fn_body[1..]
        .find("\n    fn ")
        .unwrap_or(fallback_fn_body.len());
    let fallback_fn_body = &fallback_fn_body[..next_fn];

    assert!(
        fallback_fn_body.contains("build_tab_ai_focused_part_for_view(source_view, ui_snapshot)")
            && fallback_fn_body.contains(".is_none()"),
        "ask_anything_fallback must trigger only when no focused part can be built",
    );
}

#[test]
fn search_query_and_input_targets_prevent_ambient_capture() {
    assert!(
        TAB_AI_MODE_SOURCE.contains(
            "let focused_part = if use_ask_anything_fallback || explicit_ambient_chip_label.is_some()"
        ),
        "focused-target staging must be skipped when ask-anything fallback or explicit ambient capture is active",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("tab_ai_focus_chip_staged"),
        "focused-target path must log tab_ai_focus_chip_staged",
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("tab_ai_ask_anything_fallback"),
        "ask_anything fallback path must log tab_ai_ask_anything_fallback",
    );
    assert!(
        TAB_AI_MODE_SOURCE
            .contains("use_ask_anything_fallback && explicit_ambient_chip_label.is_none()"),
        "ambient capture must remain gated to ask-anything fallback when no explicit ambient chip is requested",
    );
}

// =========================================================================
// Mandatory script verification guidance: Agent Chat path parity
// =========================================================================

#[test]
fn harness_source_builds_agent_chat_initial_input_with_guidance_before_user_intent() {
    assert!(
        HARNESS_SOURCE.contains("pub(crate) fn build_tab_ai_agent_chat_initial_input_for_prompt("),
        "harness module must define the shared Agent Chat initial-input builder"
    );
    assert!(
        HARNESS_SOURCE.contains(r#"format!("{guidance}\n\nUser intent:\n{intent}\n")"#),
        "shared Agent Chat initial-input builder must place guidance before the User intent line"
    );
    assert!(
        HARNESS_SOURCE.contains("TabAiHarnessSubmissionMode::Submit"),
        "shared Agent Chat initial-input builder must always use Submit mode"
    );
}

#[test]
fn harness_source_contains_launchpad_include() {
    // The harness module must embed START_HERE.md as the single source of
    // truth for artifact authoring guidance. If the include path changes,
    // this test catches it.
    assert!(
        HARNESS_SOURCE.contains("kit-init/examples/START_HERE.md"),
        "harness module must include START_HERE.md as the canonical launchpad source"
    );
}

// =========================================================================
// Regression: surface selection routes new-script to QuickTerminalView
// =========================================================================

/// Helper: extract the inner `begin_tab_ai_harness_entry_from_source_view`
/// function body from source.  The thin `begin_tab_ai_harness_entry` wrapper
/// delegates to this inner function, which contains the surface-preference
/// computation, routing branch, and structured log.
fn extract_begin_tab_ai_harness_entry_body() -> &'static str {
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn begin_tab_ai_harness_entry_from_source_view(")
        .expect("begin_tab_ai_harness_entry_from_source_view must exist");
    let fn_body = &TAB_AI_MODE_SOURCE[fn_start..];
    let next_fn = fn_body[1..].find("\n    fn ").unwrap_or(fn_body.len());
    &fn_body[..next_fn]
}

#[test]
fn begin_tab_ai_harness_entry_computes_surface_preference_before_routing() {
    let fn_body = extract_begin_tab_ai_harness_entry_body();
    assert!(
        fn_body.contains("tab_ai_surface_preference_for_prompt"),
        "begin_tab_ai_harness_entry must compute surface preference from shared markers"
    );
    assert!(
        fn_body.contains("surface_preference.use_quick_terminal"),
        "begin_tab_ai_harness_entry must branch on use_quick_terminal"
    );
}

#[test]
fn begin_tab_ai_harness_entry_routes_quick_terminal_to_harness_terminal() {
    let fn_body = extract_begin_tab_ai_harness_entry_body();
    assert!(
        fn_body.contains("if surface_preference.use_quick_terminal"),
        "begin_tab_ai_harness_entry must use surface_preference.use_quick_terminal as branch condition"
    );
    assert!(
        fn_body.contains("open_tab_ai_harness_terminal_from_request(request, capture_rx, cx)"),
        "when use_quick_terminal is true, must route to open_tab_ai_harness_terminal_from_request"
    );
}

#[test]
fn begin_tab_ai_harness_entry_routes_agent_chat_when_no_verification_markers() {
    let fn_body = extract_begin_tab_ai_harness_entry_body();
    assert!(
        fn_body.contains("open_tab_ai_agent_chat_view_from_request_impl("),
        "when use_quick_terminal is false, must route to Agent Chat chat"
    );
}

#[test]
fn begin_tab_ai_harness_entry_emits_surface_selected_log() {
    let fn_body = extract_begin_tab_ai_harness_entry_body();

    assert!(
        fn_body.contains(r#"event = "tab_ai_surface_selected""#),
        "begin_tab_ai_harness_entry must emit tab_ai_surface_selected log event"
    );
    assert!(
        fn_body.contains(r#"surface = if surface_preference.use_quick_terminal { "quick_terminal" } else { "agent_chat" }"#),
        "surface log field must resolve to quick_terminal or agent_chat"
    );
    assert!(
        fn_body.contains(r#"reason = if surface_preference.use_quick_terminal { "script_verification_required" } else { "default_agent_chat" }"#),
        "reason log field must resolve to script_verification_required or default_agent_chat"
    );
    assert!(
        fn_body.contains(
            "includes_script_authoring_skill = surface_preference.includes_script_authoring_skill"
        ),
        "surface_selected log must include script_authoring_skill marker"
    );
    assert!(
        fn_body.contains(
            "includes_bun_build_verification = surface_preference.includes_bun_build_verification"
        ),
        "surface_selected log must include bun_build_verification marker"
    );
    assert!(
        fn_body.contains("includes_bun_execute_verification = surface_preference.includes_bun_execute_verification"),
        "surface_selected log must include bun_execute_verification marker"
    );
}

// =========================================================================
// Regression: ScriptList submit selects QuickTerminalView (source contract)
// =========================================================================

#[test]
fn surface_preference_helper_requires_all_three_markers_for_quick_terminal() {
    // resolve_tab_ai_artifact_authoring_appendix_for_prompt must AND artifact_kind == Script
    // with all three marker flags to set use_quick_terminal = true.
    assert!(
        HARNESS_SOURCE.contains(
            "let use_quick_terminal = matches!(artifact_kind, Some(TabAiArtifactKind::Script))\n        && cached.markers.includes_script_authoring_skill\n        && cached.markers.includes_bun_build_verification\n        && cached.markers.includes_bun_execute_verification;"
        ),
        "use_quick_terminal must require Script artifact_kind and all three verification markers"
    );
}

#[test]
fn script_list_submit_forces_artifact_guidance() {
    // The deterministic ScriptList + Submit + non-empty-intent path must
    // force artifact guidance, which carries the verification markers.
    assert!(
        HARNESS_SOURCE.contains(r#"prompt_type == "ScriptList""#),
        "should_force_artifact_guidance must check for ScriptList prompt type"
    );
    assert!(
        HARNESS_SOURCE.contains("TabAiHarnessSubmissionMode::Submit"),
        "should_force_artifact_guidance must require Submit mode"
    );
}

#[test]
fn surface_preference_non_authoring_returns_all_false() {
    // The surface preference helper must return all-false when no appendix
    // is produced (the early-return path).
    assert!(
        HARNESS_SOURCE.contains(
            "return TabAiSurfacePreference {\n            use_quick_terminal: false,\n            includes_script_authoring_skill: false,\n            includes_bun_build_verification: false,\n            includes_bun_execute_verification: false,"
        ),
        "when no guidance is produced, all preference flags must be false"
    );
}

// =========================================================================
// Regression: shared guidance contains mandatory verification text
// =========================================================================

#[test]
fn harness_guidance_contains_mandatory_script_verification_header() {
    assert!(
        HARNESS_SOURCE.contains(
            r#"const SCRIPT_VERIFICATION_GATE_HEADER: &str = "MANDATORY SCRIPT VERIFICATION""#
        ),
        "harness must define MANDATORY SCRIPT VERIFICATION header constant"
    );
    // The header constant must be used in the gate builder
    assert!(
        HARNESS_SOURCE.contains("SCRIPT_VERIFICATION_GATE_HEADER"),
        "gate builder must reference the MANDATORY SCRIPT VERIFICATION header"
    );
}

#[test]
fn harness_guidance_contains_bun_build_verification_marker() {
    assert!(
        HARNESS_SOURCE.contains(
            r#"bun build ~/.scriptkit/plugins/main/scripts/<name>.ts --target=bun --outfile ~/.scriptkit/tmp/test-scripts/<name>.verify.mjs"#
        ),
        "harness must define the Bun build verification command"
    );
}

#[test]
fn harness_guidance_contains_sk_verify_execution_marker() {
    assert!(
        HARNESS_SOURCE.contains("SK_VERIFY=1 bun ~/.scriptkit/plugins/main/scripts/<name>.ts"),
        "harness must define the SK_VERIFY=1 bun execution command"
    );
}

#[test]
fn harness_guidance_contains_failure_policy() {
    assert!(
        HARNESS_SOURCE.contains("fix the script and rerun both commands"),
        "harness must include the failure policy requiring fix-and-rerun"
    );
}

#[test]
fn harness_verification_markers_are_used_in_from_guidance() {
    // TabAiVerificationGuidanceMarkers::from_guidance must check all three constants
    assert!(
        HARNESS_SOURCE.contains("SCRIPT_AUTHORING_SKILL_MARKER"),
        "from_guidance must check SCRIPT_AUTHORING_SKILL_MARKER"
    );
    assert!(
        HARNESS_SOURCE.contains("BUN_BUILD_VERIFICATION_MARKER"),
        "from_guidance must check BUN_BUILD_VERIFICATION_MARKER"
    );
    assert!(
        HARNESS_SOURCE.contains("BUN_EXECUTE_VERIFICATION_MARKER"),
        "from_guidance must check BUN_EXECUTE_VERIFICATION_MARKER"
    );
}

// =========================================================================
// Regression: Agent Chat live quick-submit reroutes verification requests
// =========================================================================

#[test]
fn agent_chat_live_quick_submit_reroutes_verification_bearing_requests_to_quick_terminal() {
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn submit_live_agent_chat_tab_ai_from_plan(")
        .expect("submit_live_agent_chat_tab_ai_from_plan must exist");
    let fn_body = &TAB_AI_MODE_SOURCE[fn_start..];
    let next_fn = fn_body[1..].find("\n    fn ").unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_fn];

    assert!(
        fn_body.contains("tab_ai_surface_preference_for_prompt"),
        "live Agent Chat quick submit must compute surface preference from the shared markers"
    );
    assert!(
        fn_body.contains("if surface_preference.use_quick_terminal"),
        "live Agent Chat quick submit must reroute verification-bearing requests to the terminal"
    );
    assert!(
        fn_body.contains("begin_tab_ai_harness_entry_from_source_view"),
        "rerouted Agent Chat quick submit must preserve the original source view"
    );
    assert!(
        fn_body.contains(r#"event = "tab_ai_quick_submit_agent_chat_live_rerouted""#),
        "rerouted Agent Chat quick submit must emit a dedicated log event"
    );
}

// =========================================================================
// Regression: unit tests for surface preference exist in harness module
// =========================================================================

#[test]
fn harness_module_has_surface_preference_unit_tests() {
    // The harness module must contain unit tests that exercise the surface
    // preference helper with ScriptList + Submit + intent → quick_terminal
    // and non-authoring flows → agent_chat.
    assert!(
        HARNESS_SOURCE.contains("fn surface_preference_script_list_submit_uses_quick_terminal"),
        "harness module must have unit test for ScriptList submit → quick_terminal"
    );
    assert!(
        HARNESS_SOURCE.contains("fn surface_preference_non_authoring_stays_agent_chat"),
        "harness module must have unit test for non-authoring → agent_chat"
    );
    assert!(
        HARNESS_SOURCE.contains("fn surface_preference_no_appendix_returns_all_false"),
        "harness module must have unit test for no-appendix → all false"
    );
    assert!(
        HARNESS_SOURCE.contains("fn surface_preference_none_intent_returns_all_false"),
        "harness module must have unit test for None intent → all false"
    );
}

// =========================================================================
// AC: Script creation flow (ScriptList + Submit) produces verification-bearing
//     guidance and selects the quick terminal path (runtime call)
// =========================================================================

#[test]
fn script_list_submit_creation_flow_selects_quick_terminal_with_all_markers() {
    // Runtime call: ScriptList + Submit + non-empty intent must produce
    // verification-bearing guidance that selects the quick terminal surface.
    let pref = script_kit_gpui::ai::tab_ai_surface_preference_for_prompt(
        "ScriptList",
        Some("clipboard cleanup"),
        script_kit_gpui::ai::TabAiHarnessSubmissionMode::Submit,
    );
    assert!(
        pref.use_quick_terminal,
        "ScriptList + Submit + intent must select quick_terminal"
    );
    assert!(
        pref.includes_script_authoring_skill,
        "script creation flow must include script_authoring_skill marker"
    );
    assert!(
        pref.includes_bun_build_verification,
        "authoring flow must include bun_build_verification marker"
    );
    assert!(
        pref.includes_bun_execute_verification,
        "authoring flow must include bun_execute_verification marker"
    );
}

// =========================================================================
// AC: Non-authoring flow omits appendix and stays on Agent Chat (runtime call)
// =========================================================================

#[test]
fn non_authoring_flow_stays_agent_chat_with_no_markers() {
    // Runtime call: FileSearch + Submit + non-artifact intent must NOT
    // produce verification-bearing guidance and must stay on Agent Chat.
    let pref = script_kit_gpui::ai::tab_ai_surface_preference_for_prompt(
        "FileSearch",
        Some("rename this file"),
        script_kit_gpui::ai::TabAiHarnessSubmissionMode::Submit,
    );
    assert!(
        !pref.use_quick_terminal,
        "non-authoring flow must stay on Agent Chat"
    );
    assert!(
        !pref.includes_script_authoring_skill,
        "non-authoring flow must not include script_authoring_skill"
    );
    assert!(
        !pref.includes_bun_build_verification,
        "non-authoring flow must not include bun_build_verification"
    );
    assert!(
        !pref.includes_bun_execute_verification,
        "non-authoring flow must not include bun_execute_verification"
    );
}

// =========================================================================
// AC: Runtime log — authoring flow emits tab_ai_artifact_authoring_guidance_appended
//     with all three verification markers set to true
// =========================================================================

#[test]
fn harness_submission_authoring_flow_emits_guidance_appended_log_with_all_markers() {
    // Source contract: build_tab_ai_harness_submission must emit the
    // tab_ai_artifact_authoring_guidance_appended log event with all
    // verification marker fields when an authoring appendix is resolved.
    let fn_start = HARNESS_SOURCE
        .find("pub fn build_tab_ai_harness_submission(")
        .expect("build_tab_ai_harness_submission must exist");
    let fn_body = &HARNESS_SOURCE[fn_start..];
    // Find the next top-level fn or end of file.
    let fn_end = fn_body[1..]
        .find("\npub fn ")
        .or_else(|| fn_body[1..].find("\npub(crate) fn "))
        .or_else(|| fn_body[1..].find("\nfn "))
        .unwrap_or(fn_body.len());
    let fn_body = &fn_body[..fn_end];

    assert!(
        fn_body.contains(r#"event = "tab_ai_artifact_authoring_guidance_appended""#),
        "harness submission must emit tab_ai_artifact_authoring_guidance_appended log"
    );
    assert!(
        fn_body.contains(
            "includes_script_authoring_skill = appendix.markers.includes_script_authoring_skill"
        ),
        "guidance_appended log must include script_authoring_skill marker from appendix"
    );
    assert!(
        fn_body.contains(
            "includes_bun_build_verification = appendix.markers.includes_bun_build_verification"
        ),
        "guidance_appended log must include bun_build_verification marker from appendix"
    );
    assert!(
        fn_body.contains(
            "includes_bun_execute_verification = appendix.markers.includes_bun_execute_verification"
        ),
        "guidance_appended log must include bun_execute_verification marker from appendix"
    );
    assert!(
        fn_body.contains("use_quick_terminal = appendix.use_quick_terminal"),
        "guidance_appended log must include use_quick_terminal from appendix"
    );
}

// =========================================================================
// AC: Runtime log — non-authoring flow does NOT emit the guidance-appended
//     event (the resolve call returns None so the log block is skipped)
// =========================================================================

#[test]
fn non_authoring_harness_submission_omits_guidance_block() {
    // Runtime call: a non-authoring submission must NOT contain the guidance
    // block in its output, confirming the guidance_appended log is never hit.
    let context = script_kit_gpui::ai::TabAiContextBlob::from_parts(
        script_kit_gpui::ai::TabAiUiSnapshot {
            prompt_type: "FileSearch".to_string(),
            input_text: Some("rename this file".to_string()),
            ..Default::default()
        },
        Default::default(),
        vec![],
        None,
        vec![],
        vec![],
        "2026-04-04T00:00:00Z".to_string(),
    );

    let output = script_kit_gpui::ai::build_tab_ai_harness_submission(
        &context,
        Some("rename this file"),
        script_kit_gpui::ai::TabAiHarnessSubmissionMode::Submit,
        None,
        None,
        &[],
    )
    .expect("submission must succeed");

    // The guidance block delimiter should NOT appear in the output.
    assert!(
        !output.contains("--- Script Kit artifact authoring guidance ---"),
        "non-authoring submission must not contain artifact authoring guidance"
    );
    // The verification gate header should NOT appear either.
    assert!(
        !output.contains("MANDATORY SCRIPT VERIFICATION"),
        "non-authoring submission must not contain verification gate"
    );
}

// =========================================================================
// Agent Chat setup-card agent confirmation refreshes inline setup state
// =========================================================================

const AGENT_CHAT_ACTION_HANDLER_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/view.rs");

#[test]
fn agent_chat_setup_confirm_refreshes_state_and_queues_retry() {
    assert!(
        AGENT_CHAT_ACTION_HANDLER_SOURCE
            .contains("self.replace_active_setup_state(next_setup, cx);"),
        "setup-agent confirmation must refresh the inline setup state",
    );
    assert!(
        AGENT_CHAT_ACTION_HANDLER_SOURCE.contains("self.queue_setup_retry_request(cx);"),
        "setup-agent confirmation must queue a runtime retry when the selected agent becomes ready",
    );
}

#[test]
fn authoring_harness_submission_contains_verification_guidance() {
    // Runtime call: a ScriptList + Submit authoring submission must contain
    // the verification guidance block with all three markers present.
    let context = script_kit_gpui::ai::TabAiContextBlob::from_parts(
        script_kit_gpui::ai::TabAiUiSnapshot {
            prompt_type: "ScriptList".to_string(),
            input_text: Some("clipboard cleanup".to_string()),
            ..Default::default()
        },
        Default::default(),
        vec![],
        None,
        vec![],
        vec![],
        "2026-04-04T00:00:00Z".to_string(),
    );

    let output = script_kit_gpui::ai::build_tab_ai_harness_submission(
        &context,
        Some("clipboard cleanup"),
        script_kit_gpui::ai::TabAiHarnessSubmissionMode::Submit,
        None,
        None,
        &[],
    )
    .expect("submission must succeed");

    // The guidance block delimiter must appear.
    assert!(
        output.contains("--- Script Kit artifact authoring guidance ---"),
        "authoring submission must contain artifact authoring guidance"
    );
    // The verification gate header must appear.
    assert!(
        output.contains("MANDATORY SCRIPT VERIFICATION"),
        "authoring submission must contain mandatory verification gate"
    );
    // All three verification markers must be present.
    assert!(
        output.contains(script_kit_gpui::ai::SCRIPT_AUTHORING_SKILL_MARKER),
        "authoring submission must include new-script skill marker"
    );
    assert!(
        output.contains(script_kit_gpui::ai::BUN_BUILD_VERIFICATION_MARKER),
        "authoring submission must include bun build verification marker"
    );
    assert!(
        output.contains(script_kit_gpui::ai::BUN_EXECUTE_VERIFICATION_MARKER),
        "authoring submission must include bun execute verification marker"
    );
    // The user intent must follow the guidance.
    assert!(
        output.contains("User intent:\nclipboard cleanup"),
        "authoring submission must include the user intent after guidance"
    );
}

// =========================================================================
// Cmd+Enter Agent Chat handoff consistency contract
// =========================================================================

#[test]
fn actions_dialog_cmd_enter_branch_precedes_plain_enter_execute() {
    // The Cmd+Enter handler must appear BEFORE the generic is_key_enter(key)
    // branch in route_key_to_actions_dialog. If it comes after, the platform
    // modifier is never checked and Cmd+Enter is swallowed as plain Enter.
    let cmd_enter_pos = ACTIONS_DIALOG_SOURCE
        .find("tab_ai_actions_dialog_cmd_enter")
        .expect("embedded actions dialog must have a tab_ai_actions_dialog_cmd_enter handler");
    let plain_enter_pos = ACTIONS_DIALOG_SOURCE
        .find("d.activate_selected(cx)")
        .expect("embedded actions dialog must have the plain Enter activate_selected path");
    assert!(
        cmd_enter_pos < plain_enter_pos,
        "Cmd+Enter Agent Chat handoff must precede the generic Enter execute path in route_key_to_actions_dialog"
    );
}

#[test]
fn detached_actions_window_has_distinct_send_to_agent_chat_intent() {
    assert!(
        ACTIONS_WINDOW_SOURCE.contains("SendToAgentChat"),
        "detached actions window must define a SendToAgentChat key intent variant"
    );
    // In the actions_window_key_intent function, the Cmd+Enter → SendToAgentChat
    // branch must appear before the plain Enter → ExecuteSelected branch.
    let intent_fn_start = ACTIONS_WINDOW_SOURCE
        .find("fn actions_window_key_intent(")
        .expect("actions_window_key_intent function must exist");
    let intent_fn = &ACTIONS_WINDOW_SOURCE[intent_fn_start..];
    let send_to_agent_chat_pos = intent_fn
        .find("ActionsWindowKeyIntent::SendToAgentChat")
        .expect("SendToAgentChat must be returned from actions_window_key_intent");
    let execute_selected_pos = intent_fn
        .find("ActionsWindowKeyIntent::ExecuteSelected")
        .expect("ExecuteSelected must be returned from actions_window_key_intent");
    assert!(
        send_to_agent_chat_pos < execute_selected_pos,
        "SendToAgentChat must be returned before ExecuteSelected in actions_window_key_intent"
    );
}

#[test]
fn detached_actions_window_routes_cmd_enter_to_send_to_agent_chat() {
    // In the key intent function, Cmd+Enter must map to SendToAgentChat
    let intent_fn_start = ACTIONS_WINDOW_SOURCE
        .find("fn actions_window_key_intent(")
        .expect("actions_window_key_intent function must exist");
    let intent_fn = &ACTIONS_WINDOW_SOURCE[intent_fn_start..];
    let next_fn = intent_fn[1..].find("\nfn ").unwrap_or(intent_fn.len());
    let intent_fn = &intent_fn[..next_fn];

    assert!(
        intent_fn.contains("modifiers.platform"),
        "actions_window_key_intent must check platform modifier for Cmd+Enter"
    );
    assert!(
        intent_fn.contains("ActionsWindowKeyIntent::SendToAgentChat"),
        "actions_window_key_intent must return SendToAgentChat for Cmd+Enter"
    );
}

#[test]
fn agent_handoff_has_explicit_target_agent_chat_open_method() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("fn open_tab_ai_agent_chat_with_explicit_target("),
        "agent_handoff must define open_tab_ai_agent_chat_with_explicit_target for Cmd+Enter handoff"
    );
    // The method must stage a FocusedTarget chip, not synthesize prompt text
    let fn_start = TAB_AI_MODE_SOURCE
        .find("fn open_tab_ai_agent_chat_with_explicit_target(")
        .expect("open_tab_ai_agent_chat_with_explicit_target must exist");
    let fn_body = &TAB_AI_MODE_SOURCE[fn_start..];
    let next_fn = fn_body[1..].find("\n    fn ").unwrap_or(fn_body.len());
    let fn_body = &fn_body[..next_fn];

    assert!(
        fn_body.contains("AiContextPart::FocusedTarget"),
        "open_tab_ai_agent_chat_with_explicit_target must stage a FocusedTarget chip"
    );
    assert!(
        fn_body.contains("tab_ai_explicit_target_agent_chat_open"),
        "open_tab_ai_agent_chat_with_explicit_target must emit the explicit-target log event"
    );
    assert!(
        fn_body.contains("submit_reused_entry_intent_with_host_context_in_detached_chat"),
        "explicit-target Agent Chat entry must reuse a live detached chat with staged host context"
    );
}

#[test]
fn ai_mod_has_build_action_target_for_ai() {
    assert!(
        AI_MOD_SOURCE_FOR_ACTIONS.contains("fn build_action_target_for_ai("),
        "ai/mod.rs must define build_action_target_for_ai for canonical action targeting"
    );
    let fn_start = AI_MOD_SOURCE_FOR_ACTIONS
        .find("fn build_action_target_for_ai(")
        .expect("build_action_target_for_ai must exist");
    let fn_body = &AI_MOD_SOURCE_FOR_ACTIONS[fn_start..];

    assert!(
        fn_body.contains("ActionsDialog"),
        "build_action_target_for_ai must set source to ActionsDialog"
    );
    assert!(
        fn_body.contains("generate_semantic_id"),
        "build_action_target_for_ai must use generate_semantic_id for the semantic ID"
    );
}

#[test]
fn embedded_actions_cmd_enter_preserves_plain_enter_behavior() {
    // The plain Enter activate_selected path must still exist and be reachable
    let route_fn_start = ACTIONS_DIALOG_SOURCE
        .find("fn route_key_to_actions_dialog(")
        .expect("route_key_to_actions_dialog must exist");
    let route_fn = &ACTIONS_DIALOG_SOURCE[route_fn_start..];

    assert!(
        route_fn.contains("activate_selected(cx)"),
        "route_key_to_actions_dialog must still contain the plain Enter activate_selected path"
    );
    assert!(
        route_fn.contains("ActionsRoute::Execute"),
        "route_key_to_actions_dialog must still return ActionsRoute::Execute for plain Enter"
    );
}

#[test]
fn close_actions_popup_checks_pending_agent_chat_target() {
    let close_fn_start = ACTIONS_DIALOG_SOURCE
        .find("fn close_actions_popup(")
        .expect("close_actions_popup must exist");
    let close_fn = &ACTIONS_DIALOG_SOURCE[close_fn_start..];

    assert!(
        close_fn.contains("take_pending_explicit_agent_chat_target"),
        "close_actions_popup must check for pending Agent Chat handoff targets from the detached window"
    );
    assert!(
        close_fn.contains("open_tab_ai_agent_chat_with_explicit_target"),
        "close_actions_popup must route pending targets to the explicit-target Agent Chat opener"
    );
}

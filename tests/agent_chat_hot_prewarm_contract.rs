use std::fs;

fn fn_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("{signature} must exist"));
    let rest = &source[start..];
    let brace = rest
        .find('{')
        .unwrap_or_else(|| panic!("{signature} must have a body"));
    let mut depth = 0usize;
    let mut end = None;
    for (idx, ch) in rest[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(brace + idx + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    &rest[..end.unwrap_or(rest.len())]
}

#[test]
fn script_list_app_has_separate_hidden_agent_chat_prewarm_slot() {
    let app_state =
        fs::read_to_string("src/main_sections/app_state.rs").expect("read app state source");
    let startup = fs::read_to_string("src/app_impl/startup.rs").expect("read startup source");

    assert!(
        app_state
            .contains("prewarmed_agent_chat: Option<Entity<crate::ai::agent_chat::ui::view::AgentChatView>>,"),
        "ScriptListApp must keep a hidden Agent Chat prewarm slot distinct from embedded_agent_chat"
    );
    assert!(
        startup.contains("prewarmed_agent_chat: None,"),
        "startup initialization must clear the hidden Agent Chat prewarm slot"
    );
}

#[test]
fn startup_schedules_agent_chat_connection_prewarm() {
    let startup = fs::read_to_string("src/app_impl/startup.rs").expect("read startup source");
    let dev_sh = fs::read_to_string("dev.sh").expect("read dev.sh");

    assert!(
        startup.contains("crate::ai::agent_chat::ui::prewarm_agent_config();"),
        "startup should still prewarm config before hot Agent Chat connection work"
    );
    assert!(
        startup.contains("this.warm_agent_chat_on_startup(cx);"),
        "startup must schedule hidden Agent Chat Chat prewarm"
    );

    let config_idx = startup
        .find("crate::ai::agent_chat::ui::prewarm_agent_config();")
        .expect("config prewarm call exists");
    let chat_idx = startup
        .find("this.warm_agent_chat_on_startup(cx);")
        .expect("Agent Chat chat warm call exists");
    assert!(
        config_idx < chat_idx,
        "agent config prewarm should be kicked off before Agent Chat chat prewarm"
    );
    assert!(
        dev_sh.contains(
            r#"SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM="${SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM:-0}""#
        ),
        "./dev.sh must enable Agent Chat hot prewarm by default while preserving explicit env opt-out"
    );
    assert!(
        !dev_sh.contains("SCRIPT_KIT_DISABLE_CODEX_AGENT_CHAT"),
        "./dev.sh must not hide Codex from the visible Agent Chat catalog; disabling hidden Agent Chat prewarm is the prompt-safe dev guard"
    );
}

#[test]
fn agent_chat_hot_prewarm_helper_uses_pi_warm_session() {
    let tab_ai =
        fs::read_to_string("src/app_impl/agent_handoff/mod.rs").expect("read tab ai source");
    let body = fn_body(&tab_ai, "pub(crate) fn warm_agent_chat_on_startup(");

    // The opt-out check moved into the shared `agent_chat_hot_prewarm_enabled`
    // helper (agent_chat_launch.rs), which reads
    // SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM; startup must gate on it
    // before spawning any Agent Chat runtime.
    assert!(
        body.contains("agent_chat_hot_prewarm_enabled()"),
        "warm_agent_chat_on_startup must honor the dev opt-out before spawning an Agent Chat runtime"
    );
    let launch = fs::read_to_string("src/app_impl/agent_handoff/agent_chat_launch.rs")
        .expect("read agent chat launch source");
    assert!(
        launch.contains("SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM"),
        "the shared hot-prewarm gate must read the dev opt-out env var"
    );
    assert!(
        body.contains("spine_cwd_for_agent_chat_launch"),
        "warm_agent_chat_on_startup must use the same cwd override as Agent Chat launch"
    );
    assert!(
        body.contains("resolve_selected_pi_launch_with_cwd_override"),
        "warm_agent_chat_on_startup must share selected-profile/cwd launch resolution with the open path"
    );
    assert!(
        body.contains("warm_session_manager()"),
        "warm_agent_chat_on_startup must route through the shared Pi warm-session manager"
    );
    assert!(
        body.contains("prepare_warm_background"),
        "startup prewarm must not block the UI update path"
    );
    assert!(
        !body.contains("cx.new(|cx| crate::ai::agent_chat::ui::AgentChatView::new("),
        "startup prewarm must not create a hidden Agent Chat view on the UI thread"
    );
}

#[test]
fn profile_selection_starts_selected_profile_warm_session() {
    let tab_ai =
        fs::read_to_string("src/app_impl/agent_handoff/mod.rs").expect("read tab ai source");
    let body = fn_body(&tab_ai, "fn select_agent_chat_profile_and_relaunch(");

    assert!(
        body.contains("persist_agent_chat_profile_selection"),
        "profile selection must persist the selected Agent Chat profile"
    );
    assert!(
        body.contains("save_user_preferences"),
        "profile selection must save preferences before warming the selected launch"
    );
    assert!(
        body.contains("prewarm_selected_agent_chat_profile_for_current_cwd"),
        "profile selection must warm the selected profile with the current cwd override before the next open"
    );
}

#[test]
fn entry_intent_does_not_reuse_cached_setup_mode_agent_chat_view() {
    let tab_ai =
        fs::read_to_string("src/app_impl/agent_handoff/mod.rs").expect("read tab ai source");
    let open_body = fn_body(&tab_ai, "fn open_tab_ai_agent_chat_with_options(");
    let reuse_body = fn_body(&tab_ai, "fn try_reuse_embedded_agent_chat_view(");
    let predicate_body = fn_body(
        &tab_ai,
        "fn should_reuse_embedded_agent_chat_view_for_open(",
    );

    assert!(
        open_body.contains("cached_agent_chat_is_setup_mode"),
        "Agent Chat open must inspect whether the cached embedded Agent Chat view is setup-mode"
    );
    assert!(
        open_body.contains("should_reuse_embedded_agent_chat_view_for_open")
            && open_body.contains("cached_agent_chat_is_setup_mode"),
        "Agent Chat open must pass setup-mode state into the reuse predicate"
    );
    assert!(
        predicate_body.contains("!cached_agent_chat_is_setup_mode"),
        "non-empty entry intents must not select setup-mode Agent Chat cache reuse"
    );
    assert!(
        reuse_body.contains("if normalized_intent.is_some() && is_setup_mode"),
        "direct setup-mode reuse must fail closed for auto-submit entry intents"
    );
    assert!(
        reuse_body.contains("self.embedded_agent_chat = None;"),
        "setup-mode cache rejection must clear the stale embedded Agent Chat view"
    );
    assert!(
        reuse_body.contains("tab_ai_embedded_agent_chat_reuse_rejected_setup_mode"),
        "setup-mode cache rejection must leave an audit log"
    );
    assert!(
        reuse_body.contains("return false;"),
        "setup-mode cache rejection must fall through to fresh launch resolution"
    );
}

#[test]
fn spine_profile_selection_starts_selected_profile_warm_session() {
    let tab_ai =
        fs::read_to_string("src/app_impl/agent_handoff/mod.rs").expect("read tab ai source");
    let body = fn_body(
        &tab_ai,
        "pub(crate) fn try_submit_spine_prompt_plan_from_parse_with_aliases(",
    );

    assert!(
        body.contains("persist_agent_chat_profile_selection"),
        "Spine profile syntax must persist the selected Agent Chat profile"
    );
    assert!(
        body.contains("prewarm_selected_agent_chat_profile_for_current_cwd"),
        "Spine profile syntax must warm the selected profile before the next Cmd+Return"
    );
}

#[test]
fn codex_agent_chat_kill_switch_is_manual_not_dev_default() {
    let config = fs::read_to_string("src/ai/agent_chat/ui/config.rs")
        .expect("read Agent Chat config source");
    let dev_sh = fs::read_to_string("dev.sh").expect("read dev.sh");

    assert!(
        config.contains("SCRIPT_KIT_DISABLE_CODEX_AGENT_CHAT"),
        "Agent Chat config must keep a manual Codex Agent Chat kill switch"
    );
    assert!(
        !dev_sh.contains("SCRIPT_KIT_DISABLE_CODEX_AGENT_CHAT"),
        "dev.sh must not set the manual Codex Agent Chat kill switch by default"
    );
    assert!(
        config.contains("should_be_implicit_codex_default: false"),
        "disabled Codex Agent Chat must not remain the implicit default"
    );
    assert!(
        config.contains("agent.id == CODEX_AGENT_CHAT_AGENT_ID")
            && config.contains("event = \"agent_chat_codex_agent_skipped\""),
        "catalog codex-agent_chat entries must be skipped when disabled"
    );
    assert!(
        config.contains("!codex_agent_chat_disabled_by_env()")
            && config.contains("agents.push(codex_agent_chat_agent_config());"),
        "built-in Codex Agent Chat auto-detection must be gated by the disable flag"
    );
}

#[test]
fn agent_chat_open_uses_pi_warm_session_without_fresh_agent_chat_runtime() {
    let agent_chat_launch = fs::read_to_string("src/app_impl/agent_handoff/agent_chat_launch.rs")
        .expect("read Agent Chat launch source");
    let body = fn_body(
        &agent_chat_launch,
        "fn open_tab_ai_agent_chat_view_from_request_impl(",
    );

    assert!(
        body.contains("open_tab_ai_pi_view_from_launch"),
        "open path must route through Pi warm Agent Chat launch"
    );
    assert!(
        !body.contains("spawn_with_approval"),
        "open path must not spawn a fresh Agent Chat runtime"
    );
}

#[test]
fn pi_tab_open_does_not_prepare_warm_synchronously_before_view_switch() {
    let agent_chat_launch = fs::read_to_string("src/app_impl/agent_handoff/agent_chat_launch.rs")
        .expect("read Agent Chat launch source");
    let body = fn_body(&agent_chat_launch, "fn open_tab_ai_pi_view_from_launch(");

    assert!(
        body.contains("acquire_ready_or_spawn_cold"),
        "Pi Tab open must first try the ready warm-session fast path"
    );
    assert!(
        body.contains("enter_embedded_agent_chat_surface"),
        "Pi Tab open must switch into the visible Agent Chat view for ready and cold-spawned leases"
    );
    assert!(
        !body.contains(".prepare_warm("),
        "Pi Tab open must not synchronously prepare warm sessions before switching views"
    );
}

#[test]
fn startup_agent_chat_prewarm_uses_background_prepare() {
    let tab_ai =
        fs::read_to_string("src/app_impl/agent_handoff/mod.rs").expect("read tab ai source");
    let body = fn_body(&tab_ai, "pub(crate) fn warm_agent_chat_on_startup(");

    assert!(body.contains("prepare_warm_background"));
    assert!(
        !body.contains(".prepare_warm("),
        "startup Pi prewarm must not synchronously prepare on the UI update path"
    );
}

#[test]
fn pi_agent_chat_escape_reset_uses_background_dismiss() {
    let tab_ai =
        fs::read_to_string("src/app_impl/agent_handoff/mod.rs").expect("read tab ai source");
    let body = fn_body(&tab_ai, "fn dismiss_agent_chat_warm_lease_background(");

    assert!(body.contains("dismiss_reset_background"));
    assert!(
        !body.contains(".dismiss_reset("),
        "embedded Agent Chat close must not synchronously prepare replacement warm sessions"
    );
}

#[test]
fn prewarm_consume_requires_matching_default_launch() {
    let tab_ai =
        fs::read_to_string("src/app_impl/agent_handoff/mod.rs").expect("read tab ai source");
    let body = fn_body(&tab_ai, "fn take_prewarmed_agent_chat_for_launch(");

    assert!(
        body.contains(
            "requirements != crate::ai::agent_chat::ui::AgentChatLaunchRequirements::default()"
        ),
        "prewarmed default sessions must not be consumed by capability-specific launches"
    );
    assert!(
        body.contains("thread_selected_agent_id.as_deref() != selected_agent_id"),
        "a prewarmed session must not be reused for a different selected agent"
    );
    assert!(
        body.contains("self.prewarmed_agent_chat = None;"),
        "successful consumption must remove the hidden prewarm slot"
    );
}

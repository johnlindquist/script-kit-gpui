//! Source-contract tests for detached actions window → Agent Chat Chat handoff.
//!
//! Locks the invariant that the detached actions window uses the shared
//! action target builder and the shared secondary-window handoff helper.

use std::fs;

#[test]
fn detached_actions_window_uses_shared_action_target_builder_and_handoff() {
    let source =
        fs::read_to_string("src/actions/window.rs").expect("Failed to read src/actions/window.rs");

    assert!(
        source.contains("build_action_target_for_ai"),
        "Detached actions window must use the shared action target builder"
    );

    assert!(
        source.contains("request_explicit_agent_chat_handoff_from_secondary_window"),
        "Detached actions window must use the shared secondary-window Agent Chat handoff helper"
    );
}

/// Behavior-level proof for the ApplyPreset reroute to current Agent Chat.
///
/// `AgentChatView::apply_preset_by_id` (the deferred handoff's ApplyPreset
/// target) resolves presets through `crate::ai::presets::resolve_agent_chat_preset`,
/// so driving that seam against the real on-disk preset store proves:
/// - the selected preset's fields reach the AgentChatView-bound handoff plan
///   (system prompt staged in the composer, preferred model selected through
///   the thread's model-picker mutation);
/// - failures report the preset-specific message the deferred handoff wraps in
///   `Failed to apply AI preset: {error}`.
///
/// The remaining routing half of the invariant — the deferred handoff always
/// opens current Agent Chat and never touches the legacy AI window — is locked
/// by `deferred_agent_chat_handoff_uses_named_failure_states` in
/// `tests/actions.rs` (absence assertions for the deleted legacy-window
/// symbols), so this test needs no legacy-window source reads of its own.
#[test]
fn apply_preset_routes_to_current_agent_chat_without_legacy_window() {
    use script_kit_gpui::setup::SK_PATH_ENV;

    static SK_PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _lock = SK_PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let kit_root = std::env::temp_dir().join(format!(
        "script-kit-wp1-preset-routing-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&kit_root);
    fs::create_dir_all(&kit_root).expect("create temp kit root");
    std::env::set_var(SK_PATH_ENV, kit_root.to_str().expect("kit root str"));

    // Seed the real preset store (`~/.scriptkit/ai-presets.json` under SK_PATH).
    fs::write(
        kit_root.join("ai-presets.json"),
        r#"[{"id":"wp1-routing-preset","name":"WP1 Routing Preset","description":"d","systemPrompt":"You are the WP1 routing preset.","icon":"star","preferredModel":"claude-wp1-test-model"}]"#,
    )
    .expect("seed preset store");

    // The selected preset reaches the AgentChatView-bound handoff plan intact.
    let plan = script_kit_gpui::ai::resolve_agent_chat_preset("wp1-routing-preset")
        .expect("known preset must resolve");
    assert_eq!(
        plan.system_prompt, "You are the WP1 routing preset.",
        "the preset's system prompt must reach the current Agent Chat composer"
    );
    assert_eq!(
        plan.preferred_model.as_deref(),
        Some("claude-wp1-test-model"),
        "the preset's preferred model must reach the current Agent Chat thread"
    );

    // Failure reports use the preset-specific message.
    let error = script_kit_gpui::ai::resolve_agent_chat_preset("wp1-missing-preset")
        .expect_err("unknown preset must fail");
    assert!(
        error.contains("Unknown AI preset") && error.contains("wp1-missing-preset"),
        "failure must name the preset that could not be applied: {error}"
    );

    std::env::remove_var(SK_PATH_ENV);
    let _ = fs::remove_dir_all(&kit_root);
}

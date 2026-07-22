//! Integration tests for generic Agent Chat onboarding, recovery, and auth-capability hooks.
//!
//! These are source-level contract tests that verify the launch path and setup
//! surfaces no longer rely on Claude-specific copy or loaders.

const TAB_AI_MODE_SOURCE: &str = include_str!("../src/app_impl/agent_handoff/mod.rs");
const AGENT_CHAT_LAUNCH_SOURCE: &str =
    include_str!("../src/app_impl/agent_handoff/agent_chat_launch.rs");
const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/view.rs");
const AGENT_CHAT_SETUP_CARD_SOURCE: &str =
    include_str!("../src/ai/agent_chat/ui/components/setup_card.rs");
const AGENT_CHAT_CONFIG_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/config.rs");
const CONFIG_TYPES_SOURCE: &str = include_str!("../src/config/types.rs");
const SETUP_MOD_SOURCE: &str = include_str!("../src/setup/mod.rs");
const RELEASE_WORKFLOW_SOURCE: &str = include_str!("../.github/workflows/release.yml");
const VERIFY_MACOS_BUNDLE_SOURCE: &str = include_str!("../scripts/verify-macos-bundle.sh");

// ── Launch path uses catalog, not Claude-only loader ───────────────────

#[test]
fn agent_handoff_uses_pi_agent_chat_launch_not_legacy_catalog_loader() {
    assert!(
        AGENT_CHAT_LAUNCH_SOURCE.contains("open_tab_ai_pi_view_from_launch"),
        "agent_handoff must route Agent Chat through Pi warm launch"
    );
    assert!(
        !AGENT_CHAT_LAUNCH_SOURCE.contains("load_agent_chat_agent_catalog_entries"),
        "agent_handoff must not use the legacy Agent Chat catalog runtime launch"
    );
}

// ── AgentChatView supports setup constructor ─────────────────────────────

#[test]
fn agent_chat_view_supports_setup_constructor() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("fn new_setup"),
        "AgentChatView must have a new_setup constructor"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("AgentChatSession::Setup"),
        "AgentChatView must support Setup session state"
    );
}

#[test]
fn agent_chat_setup_mode_blocks_script_list_picker_handoff_before_live_thread() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("event = \"agent_chat_picker_trigger_ignored_setup_mode\""),
        "launcher-triggered @ and / picker handoffs must be ignored in setup mode"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("event = \"agent_chat_set_input_ignored_setup_mode\""),
        "setup-mode Agent Chat input mutation must not call live_thread"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE
            .contains("event = \"agent_chat_composer_picker_cleared_setup_mode\""),
        "setup-mode mention refresh must clear picker state without reading live_thread"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE
            .contains("event = \"agent_chat_setup_mode_key_propagated_without_live_thread\""),
        "unhandled setup-mode keys must return before live-thread keyboard logic"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("event = \"agent_chat_escape_cancel_ignored_setup_mode\""),
        "host-level Escape cancellation must not read live_thread in setup mode"
    );
}

// (Removed: legacy Agent Chat-client auth-capability tests — they referenced a
// `CLIENT_SOURCE` include of the deleted Agent Chat client. The Agent Chat backend/client
// was removed; all sessions use the Pi backend.)

// ── AiPreferences Pi-only backend ──────────────────────

#[test]
fn agent_chat_profile_config_keeps_legacy_ai_keys() {
    for needed in [
        "selected_model_id",
        "selected_profile_name",
        "profiles: Vec<AgentChatProfile>",
        "system_prompt",
    ] {
        assert!(
            CONFIG_TYPES_SOURCE.contains(needed),
            "Agent Chat profile config must keep legacy ai key/source `{needed}`"
        );
    }

    for needed in [
        "pub enum AgentChatBackend",
        "selected_profile_id",
        "selected_backend",
        "provider",
        "append_system_prompt",
        "disable_extensions",
        "disable_skills",
        "disable_prompt_templates",
    ] {
        assert!(
            CONFIG_TYPES_SOURCE.contains(needed),
            "Agent Chat profile config must expose Pi extension `{needed}`"
        );
    }
}

#[test]
fn tab_runtime_routes_pi_profiles_without_removing_agent_chat_entrypoint() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("open_tab_ai_agent_chat_with_entry_intent"),
        "Tab Agent Chat should keep the current Agent Chat entry point"
    );
    assert!(
        (AGENT_CHAT_LAUNCH_SOURCE.contains("resolve_selected_pi_launch_with_cwd_override")
            || AGENT_CHAT_LAUNCH_SOURCE.contains("resolve_effective_profile"))
            && (AGENT_CHAT_LAUNCH_SOURCE.contains("resolve_focused_text_pi_launch")
                || AGENT_CHAT_LAUNCH_SOURCE.contains("PiAgentChatLaunch::from_profile"))
            && AGENT_CHAT_LAUNCH_SOURCE.contains("open_tab_ai_pi_view_from_launch"),
        "Tab Agent Chat launch must route selected Pi profiles through the Pi launch helper"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("dismiss_active_agent_chat_warm_lease"),
        "Pi-backed Agent Chat dismissal must reset and rewarm a fresh session"
    );
    assert!(
        TAB_AI_MODE_SOURCE.contains("closing_pi_agent_chat")
            && TAB_AI_MODE_SOURCE.contains("self.embedded_agent_chat = None;"),
        "Pi-backed Agent Chat dismissal must clear the reusable embedded chat cache"
    );
}

// (Removed: legacy per-agent preference load/persist tests — the Agent Chat backend
// was removed, so all sessions use the Pi backend with no agent preference.)

// ── Catalog loader classifies built-in agents ─────────────────────────

#[test]
fn catalog_loader_classifies_builtin_agents() {
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("AgentChatAgentSource::BuiltIn"),
        "catalog loader must classify built-in Agent Chat agents distinctly"
    );
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("classify_agent_source"),
        "catalog loader should centralize source classification"
    );
}

// ── Built-in classification distinguishes legacy Claude from built-ins ─

#[test]
fn classify_agent_source_distinguishes_legacy_from_builtin() {
    // The classifier must assign LegacyClaudeCode to "claude-code" and
    // BuiltIn to the well-known auto-detected agents.
    assert!(
        AGENT_CHAT_CONFIG_SOURCE
            .contains(r#""claude-code" => super::catalog::AgentChatAgentSource::LegacyClaudeCode"#),
        "claude-code must be classified as LegacyClaudeCode"
    );
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains(
            r#""opencode" | "codex-agent_chat" => super::catalog::AgentChatAgentSource::BuiltIn"#
        ),
        "opencode and codex-agent_chat must be classified as BuiltIn"
    );
}

// ── Codex Agent Chat appears in the selectable catalog ───────────────────────

#[test]
fn catalog_loader_merges_codex_starter_for_agent_selector() {
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("merge_catalog_with_starter_agents(&mut file)"),
        "catalog loading must merge starter Agent Chat agents so Codex appears in the selector"
    );
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("codex_agent_chat_agent_config()"),
        "starter Agent Chat agents must include the Codex Agent Chat adapter config"
    );
}

#[test]
fn codex_detection_uses_local_codex_cli_not_adapter_binary_only() {
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("codex_agent_chat_default_probe_state"),
        "catalog loading must expose a Codex default probe"
    );
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("command_exists(\"codex\")"),
        "Codex diagnostics must probe the local codex CLI"
    );
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("npx_runtime_fallback_enabled: false"),
        "Codex must record that npx runtime fallback is disabled"
    );
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("let launch_ready = adapter_ready && codex_cli_ready;")
            && AGENT_CHAT_CONFIG_SOURCE.contains("should_be_implicit_codex_default: launch_ready"),
        "Codex launch readiness must require both the resolved adapter and local codex CLI, \
         never npx or codex CLI alone"
    );
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("sibling_repo_codex_agent_chat_search_roots")
            && AGENT_CHAT_CONFIG_SOURCE.contains("std::env::current_exe()")
            && AGENT_CHAT_CONFIG_SOURCE.contains("std::env::current_dir()")
            && AGENT_CHAT_CONFIG_SOURCE.contains("parent_name == Some(\"target-agent\")"),
        "Codex adapter discovery must work in the launched app, where CARGO_MANIFEST_DIR is \
         not a reliable source of the sibling dev checkout"
    );
    assert!(
        !AGENT_CHAT_CONFIG_SOURCE.contains(
            "if command_exists(\"codex-agent_chat\") && !agents.iter().any(|a| a.id == \"codex-agent_chat\")"
        ),
        "Codex must not be discoverable only when codex-agent_chat itself is on PATH"
    );
}

#[test]
fn codex_adapter_discovery_excludes_app_bundled_binary() {
    assert!(
        !AGENT_CHAT_CONFIG_SOURCE.contains("BundledApp"),
        "Codex Agent Chat adapter discovery must not treat the app bundle as an adapter source"
    );
    assert!(
        !AGENT_CHAT_CONFIG_SOURCE.contains("bundled_codex_agent_chat_path"),
        "Codex Agent Chat adapter discovery must not probe Contents/MacOS/codex-agent_chat"
    );
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("EnvOverride")
            && AGENT_CHAT_CONFIG_SOURCE.contains("SiblingRepo")
            && AGENT_CHAT_CONFIG_SOURCE.contains("RepoLocal")
            && AGENT_CHAT_CONFIG_SOURCE.contains("Path"),
        "Codex Agent Chat adapter discovery must keep external env, dev, and PATH adapter sources"
    );
}

#[test]
fn release_workflow_does_not_embed_or_sign_codex_agent_chat() {
    for forbidden in [
        "CODEX_AGENT_CHAT_VERSION",
        "Fetch and embed codex-agent_chat binary",
        "scripts/fetch-codex-agent_chat.sh",
        "Contents/MacOS/codex-agent_chat",
    ] {
        assert!(
            !RELEASE_WORKFLOW_SOURCE.contains(forbidden),
            "release workflow must not contain bundled codex-agent_chat hook: {forbidden}"
        );
    }
}

#[test]
fn bundle_verifier_rejects_extra_macos_payloads() {
    assert!(
        VERIFY_MACOS_BUNDLE_SOURCE.contains("EXPECTED_BIN="),
        "bundle verifier must still require the main executable"
    );
    assert!(
        !VERIFY_MACOS_BUNDLE_SOURCE.contains("CODEX_AGENT_CHAT_BIN"),
        "bundle verifier must not require a codex-agent_chat executable"
    );
    assert!(
        VERIFY_MACOS_BUNDLE_SOURCE.contains("! -name 'script-kit-gpui'")
            && !VERIFY_MACOS_BUNDLE_SOURCE.contains("! -name 'codex-agent_chat'"),
        "bundle verifier must allow only the main executable in Contents/MacOS"
    );
}

#[test]
fn codex_setup_normalizes_stale_absolute_adapter_paths() {
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("looks_like_codex_agent_chat_adapter_command"),
        "Codex catalog normalization must recognize adapter commands by basename"
    );
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains(".file_name()"),
        "absolute stale codex-agent_chat paths must be detected by filename"
    );
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("missing_adapter_does_not_normalize_to_npx_runtime"),
        "config tests must pin stale absolute codex-agent_chat migration away from npx"
    );
}

#[test]
fn first_run_agent_chat_catalog_seed_includes_codex_adapter() {
    assert!(
        SETUP_MOD_SOURCE.contains(r#""id": "codex-agent_chat""#),
        "fresh installs must seed codex-agent_chat into agent_chat/agents.json"
    );
    assert!(
        SETUP_MOD_SOURCE.contains(r#""command": "codex-agent_chat""#),
        "fresh installs must seed Codex as a direct Agent Chat adapter command"
    );
    assert!(
        SETUP_MOD_SOURCE.contains(r#""args": []"#),
        "fresh installs must not route Codex through npx package args"
    );
    assert!(
        !SETUP_MOD_SOURCE.contains(r#""@zed-industries/codex-agent_chat""#),
        "fresh installs must not seed a runtime npx Codex package fallback"
    );
}

// ── Catalog builder emits per-entry structured logs ──────────────────

#[test]
fn catalog_builder_emits_per_entry_logs() {
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("agent_chat_agent_catalog_entry_built"),
        "catalog builder must emit per-entry structured log events"
    );
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("install_state = ?install_state"),
        "per-entry log must include install_state"
    );
    assert!(
        AGENT_CHAT_CONFIG_SOURCE.contains("config_state = ?config_state"),
        "per-entry log must include config_state"
    );
}

// ── AgentChatThreadInit carries selected_agent ─────────────────────────────

const AGENT_CHAT_THREAD_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/thread.rs");
const SETUP_STATE_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/setup_state.rs");

#[test]
fn agent_chat_thread_init_includes_selected_agent() {
    assert!(
        AGENT_CHAT_THREAD_SOURCE
            .contains("pub selected_agent: Option<super::catalog::AgentChatAgentCatalogEntry>"),
        "AgentChatThreadInit must carry the selected agent catalog entry"
    );
}

// ── Runtime SetupRequired arms inline setup state ────────────────────

#[test]
fn runtime_setup_required_arms_inline_setup_state() {
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("from_runtime_setup_required"),
        "AgentChatThread must convert SetupRequired events into inline setup state"
    );
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("agent_chat_runtime_setup_session_armed"),
        "runtime setup recovery must be logged"
    );
}

#[test]
fn setup_state_has_runtime_constructor() {
    assert!(
        SETUP_STATE_SOURCE.contains("fn from_runtime_setup_required"),
        "AgentChatInlineSetupState must have from_runtime_setup_required constructor"
    );
    assert!(
        SETUP_STATE_SOURCE.contains("auth_required"),
        "runtime setup constructor must handle auth_required reason"
    );
}

// ── View renders runtime setup state ─────────────────────────────────

#[test]
fn agent_chat_view_renders_runtime_setup_state() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("thread_ref.setup_state()"),
        "AgentChatView render must check thread setup_state for runtime recovery"
    );
}

// ── Launch requirements threaded into thread init ────────────────────

#[test]
fn agent_handoff_threads_launch_requirements_into_agent_chat_thread_init() {
    assert!(
        TAB_AI_MODE_SOURCE.contains("launch_requirements: requirements"),
        "agent_handoff must pass launch requirements into AgentChatThreadInit"
    );
}

#[test]
fn agent_chat_thread_init_carries_launch_requirements() {
    assert!(
        AGENT_CHAT_THREAD_SOURCE
            .contains("pub launch_requirements: super::preflight::AgentChatLaunchRequirements"),
        "AgentChatThreadInit must carry launch_requirements field"
    );
}

// ── Runtime recovery preserves launch requirements ──────────────────

#[test]
fn runtime_setup_required_preserves_launch_requirements() {
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("self.launch_requirements"),
        "runtime setup recovery must preserve launch requirements instead of resetting to default"
    );
    assert!(
        AGENT_CHAT_THREAD_SOURCE.contains("agent_chat_runtime_setup_requirements_preserved"),
        "runtime setup recovery must emit a structured preservation log"
    );
}

// ── Runtime setup only suggests capable alternatives ────────────────

#[test]
fn runtime_setup_state_only_suggests_capable_alternatives() {
    assert!(
        SETUP_STATE_SOURCE.contains("has_launchable_capable_alternative"),
        "runtime setup must only suggest switching to alternatives that satisfy launch requirements"
    );
}

// ── from_resolution is fully capability-aware ──────────────────────

#[test]
fn from_resolution_uses_can_switch_capable_for_install_branch() {
    assert!(
        SETUP_STATE_SOURCE.contains("AgentNotInstalled) if can_switch_capable"),
        "AgentNotInstalled must gate on can_switch_capable, not can_switch"
    );
}

#[test]
fn from_resolution_uses_can_switch_capable_for_auth_branch() {
    assert!(
        SETUP_STATE_SOURCE.contains("AuthenticationRequired) if can_switch_capable"),
        "AuthenticationRequired must gate on can_switch_capable, not can_switch"
    );
}

#[test]
fn from_resolution_uses_can_switch_capable_for_misconfig_branch() {
    assert!(
        SETUP_STATE_SOURCE.contains("AgentMisconfigured) if can_switch_capable"),
        "AgentMisconfigured must gate on can_switch_capable, not can_switch"
    );
}

#[test]
fn from_resolution_emits_structured_log() {
    assert!(
        SETUP_STATE_SOURCE.contains("agent_chat_setup_state_from_resolution"),
        "from_resolution must emit agent_chat_setup_state_from_resolution log event"
    );
    assert!(
        SETUP_STATE_SOURCE.contains("can_switch_capable"),
        "from_resolution log must include can_switch_capable field"
    );
}

#[test]
fn from_resolution_has_capability_gap_message_helper() {
    assert!(
        SETUP_STATE_SOURCE.contains("fn capability_gap_message"),
        "setup_state must have capability_gap_message helper"
    );
}

// ── Picker surfaces auth + capability labels ──────────────────────

#[test]
fn picker_row_includes_auth_label() {
    assert!(
        AGENT_CHAT_SETUP_CARD_SOURCE.contains("setup_agent_auth_label"),
        "picker must render auth state labels"
    );
}

#[test]
fn picker_row_includes_capability_label() {
    assert!(
        AGENT_CHAT_SETUP_CARD_SOURCE.contains("setup_agent_capability_label"),
        "picker must render capability compatibility labels"
    );
}

#[test]
fn picker_log_includes_compatible_count() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("compatible_count"),
        "picker opened log must include compatible_count"
    );
}

// ── Setup picker confirmation updates live thread ───────────────────

#[test]
fn setup_picker_confirm_updates_live_thread_selected_agent() {
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("replace_selected_agent"),
        "setup picker confirmation must update the live thread selected agent"
    );
    assert!(
        AGENT_CHAT_VIEW_SOURCE.contains("agent_chat_setup_agent_confirmed_for_runtime_recovery"),
        "setup picker confirmation must emit a structured log"
    );
}

// (Removed: legacy Agent Chat agent-preference persistence tests — the per-agent
// preference load/persist flow was deleted when the Agent Chat backend was removed.
// All sessions now use the Pi backend with provider/model selection.)

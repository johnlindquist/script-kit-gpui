use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use anyhow::{anyhow, bail, Context as _, Result};

use crate::ai::agent_chat::pi::launch_spec::PiLaunchSpec;
use crate::ai::agent_chat::pi::{PiRpcLaunchSpec, PiRpcRuntime};
use crate::ai::agent_chat::profiles::{
    resolve_effective_profile, AgentChatProfileContext, ResolvedAgentChatProfile,
    BUILTIN_QUICK_AI_PROFILE_ID, BUILTIN_TEXT_PROFILE_ID, QUICK_AI_APPEND_SYSTEM_PROMPT,
    QUICK_AI_PI_MODEL, QUICK_AI_PI_TOOLS,
};
use crate::ai::agent_chat::runtime::AgentChatConnection;
use crate::ai::agent_chat::ui::config::AgentChatModelEntry;
use crate::ai::agent_chat::warm_key::pi_warm_key;
use crate::ai::agent_chat::warm_session::{AgentChatWarmSessionManager, AgentChatWarmSessionSpec};
use crate::config::{AgentChatBackend, AiPreferences};

static WARM_SESSION_MANAGER: OnceLock<AgentChatWarmSessionManager> = OnceLock::new();

#[derive(Debug, Clone)]
pub(crate) struct PiAgentChatLaunch {
    pub profile: ResolvedAgentChatProfile,
    pub launch_spec: PiLaunchSpec,
    pub rpc_spec: PiRpcLaunchSpec,
    pub warm_key: String,
    pub cwd: PathBuf,
    pub selected_model_id: Option<String>,
    pub available_models: Vec<AgentChatModelEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct CodexQuickAiExecLaunch {
    pub(crate) profile: ResolvedAgentChatProfile,
    pub(crate) spec: crate::ai::agent_chat::codex_exec::CodexQuickAiExecSpec,
    pub(crate) cwd: PathBuf,
    pub(crate) selected_model_id: Option<String>,
    pub(crate) available_models: Vec<AgentChatModelEntry>,
}

impl CodexQuickAiExecLaunch {
    fn from_builtin_profile(profile: ResolvedAgentChatProfile) -> Result<Self> {
        let expected_tools: Vec<String> = QUICK_AI_PI_TOOLS
            .iter()
            .map(|tool| tool.to_string())
            .collect();
        if profile.id != BUILTIN_QUICK_AI_PROFILE_ID
            || profile.model.as_deref() != Some(QUICK_AI_PI_MODEL)
            || profile.append_system_prompt.as_deref() != Some(QUICK_AI_APPEND_SYSTEM_PROMPT)
            || profile.tools.as_ref() != Some(&expected_tools)
            || profile.thinking.is_some()
            || profile.no_session != Some(true)
            || profile.disable_extensions != Some(true)
            || profile.disable_skills != Some(true)
            || profile.disable_prompt_templates != Some(true)
            || profile.disable_context_files != Some(true)
        {
            bail!("quick_ai_builtin_exec_contract_mismatch")
        }
        let cwd = profile
            .cwd
            .clone()
            .unwrap_or_else(crate::setup::get_kit_path);
        let selected_model_id =
            Some(crate::ai::agent_chat::codex_exec::QUICK_AI_SELECTED_MODEL_ID.to_string());
        let spec = crate::ai::agent_chat::codex_exec::CodexQuickAiExecSpec::from_builtin_contract(
            cwd.join("codex-exec"),
        );
        let available_models = vec![AgentChatModelEntry {
            id: crate::ai::agent_chat::codex_exec::QUICK_AI_SELECTED_MODEL_ID.to_string(),
            display_name: Some(QUICK_AI_PI_MODEL.to_string()),
            context_window: None,
        }];
        Ok(Self {
            profile,
            spec,
            cwd,
            selected_model_id,
            available_models,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedQuickAiLaunch {
    CodexExec(CodexQuickAiExecLaunch),
    Pi(PiAgentChatLaunch),
}

impl PiAgentChatLaunch {
    pub(crate) fn from_profile(profile: ResolvedAgentChatProfile) -> Result<Self> {
        Self::from_profile_with_cwd_override(profile, None)
    }

    /// Resolve a launch, optionally overriding the working directory the Pi
    /// process is spawned in.
    ///
    /// The Pi RPC worker bakes its `current_dir` from the launch spec at spawn
    /// time and ignores per-turn cwd, so the user's chosen working directory
    /// (the Spine cwd chip) must be applied here — before the warm session is
    /// keyed and spawned — not via `AgentChatThread::set_cwd` afterward. Because
    /// `pi_warm_key` includes the cwd, an overridden cwd produces a distinct
    /// warm-session key so a default-cwd warm session is never reused for a
    /// different directory.
    pub(crate) fn from_profile_with_cwd_override(
        profile: ResolvedAgentChatProfile,
        cwd_override: Option<PathBuf>,
    ) -> Result<Self> {
        let mut launch_spec = PiLaunchSpec::from_profile(&profile)
            .ok_or_else(|| {
                anyhow!(
                    "Pi Agent Chat is selected, but no Pi binary was resolved. Ship Contents/MacOS/pi in the app bundle or configure ai.piBinary / SCRIPT_KIT_PI_BINARY."
                )
            })?;
        if let Some(cwd_override) = cwd_override {
            launch_spec.cwd = Some(cwd_override);
        }
        let cwd = launch_spec
            .cwd
            .clone()
            .unwrap_or_else(crate::setup::get_kit_path);
        ensure_pi_cwd(&cwd)?;
        let warm_key = pi_warm_key(&launch_spec);
        let argv_flags = pi_argv_flags(&launch_spec.argv());
        tracing::info!(
            target: "script_kit::agent_chat",
            event = "pi_agent_chat_profile_launch_resolved",
            profile_id = %profile.id,
            profile_name = %profile.name,
            profile_source = ?profile.source,
            cwd = %cwd.display(),
            warm_key = %warm_key,
            argv_flags = ?argv_flags,
        );
        let rpc_spec = PiRpcLaunchSpec::new(launch_spec.pi_binary.clone(), cwd.clone())
            .with_args(launch_spec.argv());
        let selected_model_id = pi_model_selection_id(&profile);
        let available_models = selected_model_id
            .as_ref()
            .map(|id| {
                vec![AgentChatModelEntry {
                    id: id.clone(),
                    display_name: profile.model.clone().or_else(|| Some(id.clone())),
                    context_window: None,
                }]
            })
            .unwrap_or_default();

        Ok(Self {
            profile,
            launch_spec,
            rpc_spec,
            warm_key,
            cwd,
            selected_model_id,
            available_models,
        })
    }

    pub(crate) fn warm_spec(&self) -> AgentChatWarmSessionSpec {
        let rpc_spec = self.rpc_spec.clone();
        AgentChatWarmSessionSpec {
            key: self.warm_key.clone(),
            cwd: self.cwd.clone(),
            factory: Arc::new(move || {
                let runtime = PiRpcRuntime::spawn(rpc_spec.clone())?;
                Ok(Arc::new(runtime) as Arc<dyn AgentChatConnection>)
            }),
        }
    }
}

fn pi_argv_flags(argv: &[String]) -> Vec<String> {
    argv.iter()
        .filter(|arg| arg.starts_with("--"))
        .cloned()
        .collect()
}

pub(crate) fn warm_session_manager() -> &'static AgentChatWarmSessionManager {
    WARM_SESSION_MANAGER.get_or_init(AgentChatWarmSessionManager::new)
}

pub(crate) fn resolve_selected_pi_launch(
    ai: &AiPreferences,
    ctx: &AgentChatProfileContext,
) -> Result<PiAgentChatLaunch> {
    resolve_selected_pi_launch_with_cwd_override(ai, ctx, None)
}

pub(crate) fn resolve_selected_pi_launch_with_cwd_override(
    ai: &AiPreferences,
    ctx: &AgentChatProfileContext,
    cwd_override: Option<PathBuf>,
) -> Result<PiAgentChatLaunch> {
    let profile = resolve_effective_profile(ai, ctx);
    PiAgentChatLaunch::from_profile_with_cwd_override(profile, cwd_override)
}

/// Resolve only the working directory the currently selected profile would
/// launch in, mirroring `from_profile_with_cwd_override`'s
/// `launch_spec.cwd.unwrap_or(get_kit_path())` fallback.
///
/// Unlike `resolve_selected_pi_launch`, this performs no filesystem writes
/// (`ensure_pi_cwd`/`create_dir_all`), builds no launch spec, requires no Pi
/// binary, and emits no per-call log — safe for hot paths such as building
/// the main-menu `>` cwd picker rows on every keystroke.
pub(crate) fn resolve_selected_launch_cwd(
    ai: &AiPreferences,
    ctx: &AgentChatProfileContext,
) -> PathBuf {
    let profile = resolve_effective_profile(ai, ctx);
    profile.cwd.unwrap_or_else(crate::setup::get_kit_path)
}

pub(crate) fn resolve_focused_text_pi_launch(
    ai: &AiPreferences,
    ctx: &AgentChatProfileContext,
) -> Result<PiAgentChatLaunch> {
    let text_ai = AiPreferences {
        // Deliberately drop the global model selection: the Text/rewrite mini
        // is pinned to `TEXT_PI_MODEL` (fastest Codex) so variations stream
        // back with minimal latency regardless of the Agent Chat model.
        selected_model_id: None,
        selected_profile_id: Some(BUILTIN_TEXT_PROFILE_ID.to_string()),
        selected_backend: Some(AgentChatBackend::Pi),
        pi_binary: ai.pi_binary.clone(),
        profiles: ai.profiles.clone(),
        selected_profile_name: None,
        agent_chat_notify_when_hidden: ai.agent_chat_notify_when_hidden,
        cwd: ai.cwd.clone(),
        quick_ai_profile_id: None,
    };

    PiAgentChatLaunch::from_profile(resolve_effective_profile(&text_ai, ctx))
}

/// Resolve the Quick AI launch (launcher Tab-with-text).
///
/// The profile comes from `resolve_quick_ai_profile`: the user's Shift+Tab
/// picker choice (`ai.quick_ai_profile_id`) when set, else the built-in
/// zero-context default. For the built-in default this deliberately bypasses
/// `apply_ai_fallbacks`: its model is pinned to `QUICK_AI_PI_MODEL` and must
/// not be replaced by `ai.selected_model_id`. Only the pi binary fallback
/// from preferences is honored for that default; user-picked profiles get
/// the same fallback treatment they get in Agent Chat.
pub(crate) fn resolve_quick_ai_pi_launch(
    ai: &AiPreferences,
    ctx: &AgentChatProfileContext,
) -> Result<PiAgentChatLaunch> {
    let mut profile = crate::ai::agent_chat::profiles::resolve_quick_ai_profile(ai, ctx);
    if profile.pi_binary.is_none() {
        profile.pi_binary = crate::ai::agent_chat::profiles::clean_opt(ai.pi_binary.as_deref())
            .map(crate::ai::agent_chat::pi::binary::expand_tilde_path)
            .or_else(crate::ai::agent_chat::pi::binary::default_pi_binary);
    }
    PiAgentChatLaunch::from_profile(profile)
}

/// Resolve the hidden built-in Quick AI profile to cold direct Codex exec.
/// Any explicit Shift+Tab/configured profile remains on the existing Pi path.
pub(crate) fn resolve_quick_ai_launch(
    ai: &AiPreferences,
    ctx: &AgentChatProfileContext,
) -> Result<ResolvedQuickAiLaunch> {
    let profile = crate::ai::agent_chat::profiles::resolve_quick_ai_profile(ai, ctx);
    if profile.id == BUILTIN_QUICK_AI_PROFILE_ID {
        return CodexQuickAiExecLaunch::from_builtin_profile(profile)
            .map(ResolvedQuickAiLaunch::CodexExec);
    }
    let mut profile = profile;
    if profile.pi_binary.is_none() {
        profile.pi_binary = crate::ai::agent_chat::profiles::clean_opt(ai.pi_binary.as_deref())
            .map(crate::ai::agent_chat::pi::binary::expand_tilde_path)
            .or_else(crate::ai::agent_chat::pi::binary::default_pi_binary);
    }
    PiAgentChatLaunch::from_profile(profile).map(ResolvedQuickAiLaunch::Pi)
}

fn pi_model_selection_id(profile: &ResolvedAgentChatProfile) -> Option<String> {
    let provider = profile.provider.as_deref()?.trim();
    let model = profile.model.as_deref()?.trim();
    if provider.is_empty() || model.is_empty() {
        return None;
    }
    Some(format!("{provider}/{model}"))
}

fn ensure_pi_cwd(cwd: &PathBuf) -> Result<()> {
    std::fs::create_dir_all(cwd)
        .with_context(|| format!("Failed to prepare Pi Agent Chat cwd {}", cwd.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_chat::profiles::{
        built_in_general_profile, built_in_script_kit_profile, built_in_text_profile,
    };

    fn ctx() -> AgentChatProfileContext {
        AgentChatProfileContext {
            kit_path: PathBuf::from("/tmp/kit"),
        }
    }

    /// Returns the value following a `--flag value` pair in an argv vector.
    fn argv_value<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
        argv.windows(2)
            .find(|pair| pair[0] == flag)
            .map(|pair| pair[1].as_str())
    }

    #[test]
    fn text_profile_allows_only_web_search_and_no_file_access() {
        let profile = built_in_text_profile(&ctx());
        assert_eq!(profile.tools, Some(vec!["web_search".to_string()]));
        assert_eq!(
            profile.tool_policy.as_ref().and_then(|p| p.allow.clone()),
            Some(vec!["web_search".to_string()])
        );
        let path_policy = profile.path_policy.as_ref().expect("path policy");
        assert_eq!(path_policy.allow_read.as_deref(), Some(&[][..]));
        assert_eq!(path_policy.allow_write.as_deref(), Some(&[][..]));
        assert_eq!(profile.disable_extensions, Some(true));
        assert_eq!(profile.disable_skills, Some(true));
        assert_eq!(profile.disable_prompt_templates, Some(true));
        assert_eq!(profile.disable_context_files, Some(true));
        assert_eq!(profile.no_session, Some(true));
    }

    #[test]
    fn pi_launch_from_profile_builds_rpc_spec_and_warm_key() {
        let ai = AiPreferences {
            pi_binary: Some("/tmp/test-pi".to_string()),
            ..AiPreferences::default()
        };
        let launch = resolve_selected_pi_launch(&ai, &ctx()).unwrap();

        assert_eq!(launch.profile.name, "Brain");
        assert_eq!(launch.cwd, PathBuf::from("/tmp/kit/agent-chat/brain"));
        assert_eq!(launch.rpc_spec.command, PathBuf::from("/tmp/test-pi"));
        assert!(launch.rpc_spec.args.contains(&"--mode".to_string()));
        assert!(launch.rpc_spec.args.contains(&"rpc".to_string()));
        assert!(launch.warm_key.starts_with("pi-warm-v1:"));
    }

    #[test]
    fn selected_pi_launch_with_cwd_override_uses_selected_profile_key_material() {
        let ai = AiPreferences {
            pi_binary: Some("/tmp/test-pi".to_string()),
            selected_profile_id: Some(
                crate::ai::agent_chat::profiles::BUILTIN_SCRIPT_KIT_PROFILE_ID.to_string(),
            ),
            ..AiPreferences::default()
        };
        let cwd = PathBuf::from("/tmp/script-kit-work");

        let launch =
            resolve_selected_pi_launch_with_cwd_override(&ai, &ctx(), Some(cwd.clone())).unwrap();

        assert_eq!(launch.profile.id, "script-kit");
        assert_eq!(launch.cwd, cwd);
        assert!(launch.warm_key.starts_with("pi-warm-v1:"));
        assert!(
            crate::ai::agent_chat::warm_key::normalized_material(&launch.launch_spec)
                .contains("cwd=/tmp/script-kit-work")
        );
    }

    #[test]
    fn pi_launch_exposes_provider_scoped_model_for_selector_and_rpc() {
        let ai = AiPreferences {
            pi_binary: Some("/tmp/test-pi".to_string()),
            selected_profile_id: Some(
                crate::ai::agent_chat::profiles::BUILTIN_SCRIPT_KIT_PROFILE_ID.to_string(),
            ),
            ..AiPreferences::default()
        };
        let launch = resolve_selected_pi_launch(&ai, &ctx()).unwrap();

        assert_eq!(
            launch.selected_model_id.as_deref(),
            Some("openai-codex/gpt-5.6-sol")
        );
        assert_eq!(launch.available_models.len(), 1);
        assert_eq!(launch.available_models[0].id, "openai-codex/gpt-5.6-sol");
        assert_eq!(
            launch.available_models[0].display_name.as_deref(),
            Some("gpt-5.6-sol")
        );
        assert_eq!(
            argv_value(&launch.rpc_spec.args, "--thinking"),
            Some("medium")
        );
    }

    #[test]
    fn legacy_agent_chat_backend_resolves_to_pi() {
        let ai = AiPreferences {
            pi_binary: Some("/tmp/test-pi".to_string()),
            selected_backend: Some(AgentChatBackend::Pi),
            ..AiPreferences::default()
        };

        let launch = resolve_selected_pi_launch(&ai, &ctx()).unwrap();
        assert_eq!(launch.profile.backend, AgentChatBackend::Pi);
        assert_eq!(launch.profile.id, "brain");
    }

    #[test]
    fn focused_text_pi_launch_uses_text_profile_for_isolated_warm_profile() {
        let ai = AiPreferences {
            pi_binary: Some("/tmp/test-pi".to_string()),
            selected_profile_id: Some(
                crate::ai::agent_chat::profiles::BUILTIN_SCRIPT_KIT_PROFILE_ID.to_string(),
            ),
            ..AiPreferences::default()
        };

        let launch = resolve_focused_text_pi_launch(&ai, &ctx()).unwrap();
        let argv = launch.launch_spec.argv();

        assert_eq!(launch.profile.id, BUILTIN_TEXT_PROFILE_ID);
        assert_eq!(launch.profile.name, "Text");
        assert_eq!(launch.cwd, PathBuf::from("/tmp/kit/agent-chat/text"));
        // Pinned to the fastest Codex model; the global model selection must
        // not leak into the rewrite mini (speed is the surface contract).
        assert_eq!(
            launch.selected_model_id.as_deref(),
            Some("openai-codex/gpt-5.3-codex-spark")
        );
        assert_eq!(argv_value(&argv, "--thinking"), None);
        // The Text/mini profile now ships exactly one read-only network tool so
        // live-info questions can search the web; it must NOT fall back to
        // --no-tools, and must stay otherwise locked down.
        assert!(!argv.contains(&"--no-tools".to_string()));
        assert_eq!(argv_value(&argv, "--tools"), Some("web_search"));
        assert!(argv.contains(&"--no-extensions".to_string()));
        assert!(argv.contains(&"--no-skills".to_string()));
        assert!(argv.contains(&"--no-prompt-templates".to_string()));
        assert!(argv.contains(&"--no-context-files".to_string()));
        assert!(argv.contains(&"--no-session".to_string()));
        assert_eq!(
            launch.launch_spec.append_system_prompt.as_deref(),
            Some(crate::ai::agent_chat::profiles::TEXT_APPEND_SYSTEM_PROMPT)
        );
        assert_eq!(launch.launch_spec.system_prompt, None);
    }

    #[test]
    fn focused_text_pi_launch_does_not_inherit_agent_chat_prompts() {
        let ai = AiPreferences {
            pi_binary: Some("/tmp/test-pi".to_string()),
            selected_profile_id: Some("custom-pi".to_string()),
            profiles: vec![crate::config::AgentChatProfile {
                id: Some("custom-pi".to_string()),
                name: "Custom Pi".to_string(),
                backend: Some(AgentChatBackend::Pi),
                provider: Some("openai-codex".to_string()),
                model: Some("gpt-5.4".to_string()),
                system_prompt: Some("normal chat system prompt".to_string()),
                append_system_prompt: Some("normal chat append prompt".to_string()),
                ..Default::default()
            }],
            ..AiPreferences::default()
        };

        let launch = resolve_focused_text_pi_launch(&ai, &ctx()).unwrap();
        let argv = launch.launch_spec.argv();

        assert_eq!(launch.profile.id, BUILTIN_TEXT_PROFILE_ID);
        assert_eq!(launch.launch_spec.system_prompt, None);
        assert_eq!(
            launch.launch_spec.append_system_prompt.as_deref(),
            Some(crate::ai::agent_chat::profiles::TEXT_APPEND_SYSTEM_PROMPT)
        );
        assert!(!argv.contains(&"--system-prompt".to_string()));
        assert!(!argv.iter().any(|arg| arg == "normal chat system prompt"));
        assert!(!argv.iter().any(|arg| arg == "normal chat append prompt"));
    }

    #[test]
    fn focused_text_pi_launch_ignores_global_model_override() {
        // The rewrite mini is a speed surface: even with a user-selected
        // Agent Chat model, the Text profile stays pinned to the fastest
        // Codex model (same contract as Quick AI).
        let ai = AiPreferences {
            pi_binary: Some("/tmp/test-pi".to_string()),
            selected_model_id: Some("claude-sonnet".to_string()),
            ..AiPreferences::default()
        };

        let launch = resolve_focused_text_pi_launch(&ai, &ctx()).unwrap();

        assert_eq!(launch.profile.id, BUILTIN_TEXT_PROFILE_ID);
        assert_eq!(launch.profile.backend, AgentChatBackend::Pi);
        assert_eq!(launch.profile.provider.as_deref(), Some("openai-codex"));
        assert_eq!(
            launch.profile.model.as_deref(),
            Some(crate::ai::agent_chat::profiles::TEXT_PI_MODEL)
        );
        assert_eq!(launch.cwd, PathBuf::from("/tmp/kit/agent-chat/text"));
    }

    #[test]
    fn quick_ai_pi_launch_is_zero_context_and_pins_spark_model() {
        // Even with a user-selected model, Quick AI must stay pinned to the
        // spark model and strip every context surface. web_search is the one
        // allowed tool so live-info questions get real answers.
        let ai = AiPreferences {
            pi_binary: Some("/tmp/test-pi".to_string()),
            selected_model_id: Some("openai-codex/gpt-5.6-terra".to_string()),
            selected_profile_id: Some(
                crate::ai::agent_chat::profiles::BUILTIN_SCRIPT_KIT_PROFILE_ID.to_string(),
            ),
            ..AiPreferences::default()
        };

        let launch = resolve_quick_ai_pi_launch(&ai, &ctx()).unwrap();
        let argv = launch.launch_spec.argv();

        assert_eq!(
            launch.profile.id,
            crate::ai::agent_chat::profiles::BUILTIN_QUICK_AI_PROFILE_ID
        );
        assert_eq!(
            launch.selected_model_id.as_deref(),
            Some("openai-codex/gpt-5.3-codex-spark")
        );
        assert_eq!(argv_value(&argv, "--thinking"), None);
        assert_eq!(launch.cwd, PathBuf::from("/tmp/kit/agent-chat/quick-ai"));
        assert!(!argv.contains(&"--no-tools".to_string()));
        assert_eq!(argv_value(&argv, "--tools"), Some("web_search"));
        assert!(argv.contains(&"--no-extensions".to_string()));
        assert!(argv.contains(&"--no-skills".to_string()));
        assert!(argv.contains(&"--no-prompt-templates".to_string()));
        assert!(argv.contains(&"--no-context-files".to_string()));
        assert!(argv.contains(&"--no-session".to_string()));
        assert_eq!(
            launch.launch_spec.append_system_prompt.as_deref(),
            Some(crate::ai::agent_chat::profiles::QUICK_AI_APPEND_SYSTEM_PROMPT)
        );
        assert_eq!(launch.launch_spec.system_prompt, None);
    }

    #[test]
    fn quick_ai_pi_launch_honors_shift_tab_profile_pick() {
        // A profile assigned via the Shift+Tab Profile Search (Tab = "Use for
        // Quick AI") replaces the pinned zero-context default wholesale.
        let ai = AiPreferences {
            pi_binary: Some("/tmp/test-pi".to_string()),
            quick_ai_profile_id: Some(
                crate::ai::agent_chat::profiles::BUILTIN_BRAIN_PROFILE_ID.to_string(),
            ),
            ..AiPreferences::default()
        };

        let launch = resolve_quick_ai_pi_launch(&ai, &ctx()).unwrap();
        assert_eq!(
            launch.profile.id,
            crate::ai::agent_chat::profiles::BUILTIN_BRAIN_PROFILE_ID
        );
        assert_eq!(launch.cwd, PathBuf::from("/tmp/kit/agent-chat/brain"));
    }

    #[test]
    fn quick_ai_profile_is_not_a_pickable_built_in() {
        // Quick AI is a launch mode, not a profile-picker entry.
        let ids: Vec<String> = crate::ai::agent_chat::profiles::built_in_profiles(&ctx())
            .into_iter()
            .map(|profile| profile.id)
            .collect();
        assert!(!ids
            .contains(&crate::ai::agent_chat::profiles::BUILTIN_QUICK_AI_PROFILE_ID.to_string()));
    }

    #[test]
    fn codex_quick_ai_builtin_resolves_to_direct_exec_without_warm_spec() {
        let ai = AiPreferences {
            selected_model_id: Some("openai-codex/gpt-5.6-terra".to_string()),
            ..AiPreferences::default()
        };
        let ResolvedQuickAiLaunch::CodexExec(launch) =
            resolve_quick_ai_launch(&ai, &ctx()).unwrap()
        else {
            panic!("hidden built-in Quick AI must use direct Codex exec")
        };
        assert_eq!(launch.profile.id, BUILTIN_QUICK_AI_PROFILE_ID);
        assert_eq!(launch.spec.model, QUICK_AI_PI_MODEL);
        assert_eq!(
            launch.spec.developer_instructions,
            QUICK_AI_APPEND_SYSTEM_PROMPT
        );
        assert_eq!(
            launch.selected_model_id.as_deref(),
            Some(crate::ai::agent_chat::codex_exec::QUICK_AI_SELECTED_MODEL_ID)
        );
    }

    #[test]
    fn codex_quick_ai_explicit_profile_override_resolves_to_pi() {
        let ai = AiPreferences {
            pi_binary: Some("/tmp/test-pi".to_string()),
            quick_ai_profile_id: Some(
                crate::ai::agent_chat::profiles::BUILTIN_BRAIN_PROFILE_ID.to_string(),
            ),
            ..AiPreferences::default()
        };
        let ResolvedQuickAiLaunch::Pi(launch) = resolve_quick_ai_launch(&ai, &ctx()).unwrap()
        else {
            panic!("explicit Quick AI profile must remain on Pi")
        };
        assert_eq!(
            launch.profile.id,
            crate::ai::agent_chat::profiles::BUILTIN_BRAIN_PROFILE_ID
        );
    }

    #[test]
    fn codex_quick_ai_stale_profile_fallback_resolves_to_builtin_exec() {
        let ai = AiPreferences {
            quick_ai_profile_id: Some("deleted-profile".to_string()),
            ..AiPreferences::default()
        };
        assert!(matches!(
            resolve_quick_ai_launch(&ai, &ctx()).unwrap(),
            ResolvedQuickAiLaunch::CodexExec(_)
        ));
    }

    #[test]
    fn pi_launch_creates_profile_cwd_before_spawn() {
        let temp = tempfile::tempdir().expect("temp dir");
        let kit_path = temp.path().join(".scriptkit");
        let ctx = AgentChatProfileContext {
            kit_path: kit_path.clone(),
        };
        let ai = AiPreferences {
            pi_binary: Some("/tmp/test-pi".to_string()),
            ..AiPreferences::default()
        };

        let launch = resolve_selected_pi_launch(&ai, &ctx).unwrap();

        assert_eq!(launch.cwd, kit_path.join("agent-chat").join("brain"));
        assert!(launch.cwd.is_dir());
    }

    #[test]
    fn pi_argv_flags_receipt_omits_prompt_and_argument_values() {
        let flags = pi_argv_flags(&[
            "--mode".to_string(),
            "rpc".to_string(),
            "--append-system-prompt".to_string(),
            "secret prompt text".to_string(),
            "--no-context-files".to_string(),
        ]);

        assert_eq!(
            flags,
            vec![
                "--mode".to_string(),
                "--append-system-prompt".to_string(),
                "--no-context-files".to_string()
            ]
        );
    }
}

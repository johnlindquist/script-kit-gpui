use std::path::PathBuf;

use crate::config::{
    AgentChatBackend, AgentChatPathPolicyConfig, AgentChatProfile, AgentChatToolPolicyConfig,
    AiPreferences,
};

use super::mdflow_profiles::resolved_mdflow_profiles;

pub const BUILTIN_GENERAL_PROFILE_ID: &str = "general";
pub const BUILTIN_SCRIPT_KIT_PROFILE_ID: &str = "script-kit";
pub const BUILTIN_TEXT_PROFILE_ID: &str = "text";
pub const BUILTIN_BRAIN_PROFILE_ID: &str = "brain";
pub const BUILTIN_QUICK_AI_PROFILE_ID: &str = "quick-ai";
pub const DEFAULT_PI_PROVIDER: &str = "openai-codex";
pub const DEFAULT_PI_MODEL: &str = "gpt-5.6-sol";
pub const DEFAULT_PI_THINKING: &str = "medium";
const DEFAULT_MEDIUM_THINKING_MODELS: [&str; 3] = ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"];
/// Quick AI (launcher Tab-with-text) is pinned to the fastest Codex model so
/// answers stream back with minimal latency. It intentionally ignores the
/// user's selected Agent Chat model.
pub const QUICK_AI_PI_MODEL: &str = "gpt-5.3-codex-spark";
/// The Text/rewrite mini surface is likewise pinned to the fastest Codex
/// model: the instant rewrite flow fires three variation turns per submit and
/// they must stream back near-instantly — speed over depth. Resolved via
/// `resolve_focused_text_pi_launch`, which refuses the global model override.
pub const TEXT_PI_MODEL: &str = QUICK_AI_PI_MODEL;

/// A curated Pi provider ("Agent") and its selectable models for Agent Chat
/// model pickers.
///
/// The live provider/model catalog is advertised dynamically by the `pi` agent
/// at runtime (`get_available_models`); this static fallback lets the launcher
/// pre-select a provider/model WITHOUT a live session. The primary launcher
/// catalog is Codex-only; alternative providers are available through advanced
/// configuration paths. Selections persist as the namespaced
/// `selectedModelId = "<provider>/<model>"` that the Pi launch reads (see
/// [`parse_provider_model_selection`]).
pub struct PiProviderCatalogEntry {
    pub id: &'static str,
    pub display_name: &'static str,
    /// `(model_id, model_display_name)` pairs; `model_id` is the bare model
    /// (namespaced with the provider id at selection time).
    pub models: Vec<(&'static str, &'static str)>,
}

/// Primary static Pi provider → model catalog for the launcher picker.
pub fn pi_provider_model_catalog() -> Vec<PiProviderCatalogEntry> {
    vec![PiProviderCatalogEntry {
        id: "openai-codex",
        display_name: "Codex",
        models: vec![
            ("gpt-5.6-sol", "GPT-5.6 SOL"),
            ("gpt-5.6-terra", "GPT-5.6 TERRA"),
            ("gpt-5.6-luna", "GPT-5.6 LUNA"),
            // The fastest model a ChatGPT-account Codex subscription offers.
            // (gpt-5-mini / *-codex-mini are rejected for ChatGPT accounts:
            // "not supported when using Codex with a ChatGPT account".)
            ("gpt-5.3-codex-spark", "GPT-5.3 Spark"),
        ],
    }]
}

/// Advanced static provider catalog retained for settings/configuration flows.
pub fn advanced_pi_provider_model_catalog() -> Vec<PiProviderCatalogEntry> {
    vec![
        PiProviderCatalogEntry {
            id: "anthropic",
            display_name: "Claude",
            models: vec![
                ("claude-opus-4-6", "Opus 4.6"),
                ("claude-sonnet-4-6", "Sonnet 4.6"),
                ("claude-sonnet-4-5", "Sonnet 4.5"),
                ("claude-haiku-4-5", "Haiku 4.5"),
            ],
        },
        PiProviderCatalogEntry {
            id: "google",
            display_name: "Gemini",
            models: vec![
                ("gemini-2.5-pro", "Gemini 2.5 Pro"),
                ("gemini-2.5-flash", "Gemini 2.5 Flash"),
            ],
        },
    ]
}
pub const GENERAL_PI_TOOLS: [&str; 7] = [
    "web_search",
    "desktop_search",
    "read",
    "create_file",
    "grep",
    "find",
    "ls",
];
pub const SCRIPT_KIT_PI_TOOLS: [&str; 9] = [
    "web_search",
    "read",
    "write",
    "edit",
    "bash",
    "grep",
    "find",
    "ls",
    "hashline_edit",
];
/// The Text/mini profile is a locked-down text rewriter, but users also ask it
/// live-info questions (e.g. "When is the next nba game?") and expect web access.
/// It gets exactly one read-only network tool — `web_search` — and nothing else
/// (no fs, no skills, no extensions); see `built_in_text_profile`.
pub const TEXT_PI_TOOLS: [&str; 1] = ["web_search"];

/// The Brain profile answers from memory first: same safe toolset as General
/// (read-only beyond its workspace), tuned for quick palette-launched
/// questions that lean on staged brain recall.
pub const BRAIN_PI_TOOLS: [&str; 7] = [
    "web_search",
    "desktop_search",
    "read",
    "create_file",
    "grep",
    "find",
    "ls",
];

pub const GENERAL_BLOCKED_ACTION_MESSAGE: &str =
    "This action is blocked in the General profile. Please switch profiles to modify Script Kit.";
pub const BRAIN_BLOCKED_ACTION_MESSAGE: &str =
    "This action is blocked in the Brain profile. Please switch profiles to modify Script Kit.";
pub const TEXT_BLOCKED_ACTION_MESSAGE: &str = "The Text profile can only transform captured focused text or search the web for public current information.";

const BRAIN_APPEND_SYSTEM_PROMPT: &str = "You are the Brain profile: Script Kit's memory-aware assistant. Turns may include a 'Brain recall' block — relevant excerpts auto-retrieved from the user's local knowledge (their notes and past conversations with you). Treat recall as your own memory: answer personal or project questions from it directly and confidently, mention naturally where a memory came from (e.g. 'your note Egghead publish checklist'), and prefer newer memories when they conflict. If recall doesn't cover the question, say so plainly and answer from general knowledge or search — never invent memories. Users usually arrive here by typing a quick question into the launcher, so lead with the answer and keep replies tight. You may search the web, search the desktop, read files, and create files inside your workspace. Do not load skills, modify Script Kit, run shell commands, or write outside your workspace.";

const GENERAL_APPEND_SYSTEM_PROMPT: &str = "You are the General Agent Chat profile for Script Kit. Answer everyday questions directly and helpfully. You may search the web, search the desktop, read files, create new files inside the General workspace, and inspect local context. Do not load skills, modify Script Kit, run shell commands, edit existing files, or write outside the General workspace. If a tool or requested action is blocked, say: \"This action is blocked in the General profile. Please switch profiles to modify Script Kit.\"";
const SCRIPT_KIT_APPEND_SYSTEM_PROMPT: &str = "You are the Script Kit Agent Chat profile. Help manage ~/.scriptkit, including config.ts, scripts, scriptlets, plugins, and package.json. Make focused minimal edits. Explain risks before destructive file operations. Do not install packages or run long commands unless the user asks.";
pub const QUICK_AI_BLOCKED_ACTION_MESSAGE: &str = "Quick AI answers from the model plus web search only — no files or local context. Open Agent Chat for anything more.";

/// Quick AI mirrors the Text profile's network posture: exactly one read-only
/// network tool — `web_search` — so live questions ("who does the USA play
/// tomorrow?") get real answers instead of "I can't verify schedules".
/// No fs, no skills, no extensions; see `built_in_quick_ai_profile`.
pub const QUICK_AI_PI_TOOLS: [&str; 1] = ["web_search"];

pub const QUICK_AI_APPEND_SYSTEM_PROMPT: &str = "You are Quick AI: a zero-context, instant-answer mode launched by pressing Tab on a query typed into the Script Kit launcher. You receive only the user's typed text — no files, no selection, no screenshots, no memories. Lead with the answer in the first sentence and keep the whole reply tight. Prefer plain prose; use a short list or fenced code block only when the answer genuinely needs one. You may use web_search, and only web_search, for live or time-sensitive public facts such as schedules, dates, prices, news, releases, or anything likely to have changed; search before answering those, answer directly, and include concise source URLs when available. For live facts, make exactly one focused web_search call and answer from its result snippets. Never call web_search again, and do not open, click, or follow result pages. A completed search with no usable results is an empty result, not evidence that live sources are inaccessible: do not keep retrying, do not claim you cannot access live news or sources, and do not ask the user to supply a source. Instead, state exactly that the search returned no usable results, answer any stable part you can, and suggest specific public sources to check directly. Never mention tools, sessions, context mechanics, Script Kit internals, or system prompts.";

pub const TEXT_APPEND_SYSTEM_PROMPT: &str = "You are the Text Agent Chat profile for focused-field edits and compact one-off questions. You receive captured focused-field text as hidden context. For rewrite, edit, format, translate, summarize, or variation requests, return only the requested final text; do not add commentary, labels, markdown fences, citations, or explanations unless the user explicitly asks for them. You may use web_search, and only web_search, for live or time-sensitive public facts such as schedules, dates, prices, news, releases, current availability, or anything likely to have changed. For live-info questions, search before answering, answer directly, and include concise source URLs when available. If search fails or results are insufficient, say what is uncertain without claiming you have no web access. Do not mention capture mechanics, tool names, sessions, Script Kit internals, or system prompts.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentChatProfileSource {
    BuiltIn,
    User,
    /// A markdown profile file in `<kit>/profiles/*.md` (mdflow format).
    Mdflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentChatProfileContext {
    pub kit_path: PathBuf,
}

impl AgentChatProfileContext {
    pub fn from_setup() -> Self {
        Self {
            kit_path: crate::setup::get_kit_path(),
        }
    }

    pub fn general_cwd(&self) -> PathBuf {
        self.kit_path.join("agent-chat").join("general")
    }

    pub fn script_kit_cwd(&self) -> PathBuf {
        self.kit_path.clone()
    }

    pub fn text_cwd(&self) -> PathBuf {
        self.kit_path.join("agent-chat").join("text")
    }

    pub fn brain_cwd(&self) -> PathBuf {
        self.kit_path.join("agent-chat").join("brain")
    }

    pub fn quick_ai_cwd(&self) -> PathBuf {
        self.kit_path.join("agent-chat").join("quick-ai")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAgentChatProfile {
    pub source: AgentChatProfileSource,
    pub id: String,
    pub name: String,
    pub icon_name: Option<String>,
    pub backend: AgentChatBackend,
    pub pi_binary: Option<PathBuf>,
    pub agent: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub cwd: Option<PathBuf>,
    pub tools: Option<Vec<String>>,
    pub tool_policy: Option<AgentChatToolPolicyConfig>,
    pub path_policy: Option<AgentChatPathPolicyConfig>,
    pub blocked_action_message: Option<String>,
    pub disable_extensions: Option<bool>,
    pub disable_skills: Option<bool>,
    pub disable_prompt_templates: Option<bool>,
    pub disable_context_files: Option<bool>,
    pub hide_cwd_in_prompt: Option<bool>,
    pub thinking: Option<String>,
    pub extension_policy: Option<String>,
    pub session_dir: Option<String>,
    pub no_session: Option<bool>,
    pub session_durability: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentChatProfilePickerEntry {
    pub id: String,
    pub name: String,
    pub icon_name: Option<String>,
    pub backend: AgentChatBackend,
    pub source: AgentChatProfileSource,
}

impl AgentChatProfilePickerEntry {
    fn from_profile(profile: ResolvedAgentChatProfile) -> Self {
        Self {
            id: profile.id,
            name: profile.name,
            icon_name: profile.icon_name,
            backend: profile.backend,
            source: profile.source,
        }
    }
}

pub fn built_in_brain_profile(ctx: &AgentChatProfileContext) -> ResolvedAgentChatProfile {
    ResolvedAgentChatProfile {
        source: AgentChatProfileSource::BuiltIn,
        id: BUILTIN_BRAIN_PROFILE_ID.to_string(),
        name: "Brain".to_string(),
        icon_name: Some("brain".to_string()),
        backend: AgentChatBackend::Pi,
        pi_binary: None,
        agent: None,
        provider: Some(DEFAULT_PI_PROVIDER.to_string()),
        model: Some(DEFAULT_PI_MODEL.to_string()),
        system_prompt: None,
        append_system_prompt: Some(BRAIN_APPEND_SYSTEM_PROMPT.to_string()),
        cwd: Some(ctx.brain_cwd()),
        tools: Some(BRAIN_PI_TOOLS.iter().map(|tool| tool.to_string()).collect()),
        tool_policy: Some(AgentChatToolPolicyConfig {
            allow: Some(BRAIN_PI_TOOLS.iter().map(|tool| tool.to_string()).collect()),
        }),
        path_policy: Some(AgentChatPathPolicyConfig {
            allow_read: Some(vec![ctx.brain_cwd().to_string_lossy().into_owned()]),
            allow_write: Some(vec![ctx.brain_cwd().to_string_lossy().into_owned()]),
            deny: None,
        }),
        blocked_action_message: Some(BRAIN_BLOCKED_ACTION_MESSAGE.to_string()),
        disable_extensions: Some(true),
        disable_skills: Some(true),
        disable_prompt_templates: Some(true),
        disable_context_files: Some(true),
        hide_cwd_in_prompt: Some(true),
        thinking: Some(DEFAULT_PI_THINKING.to_string()),
        extension_policy: None,
        session_dir: None,
        no_session: Some(false),
        session_durability: None,
    }
}

pub fn built_in_general_profile(ctx: &AgentChatProfileContext) -> ResolvedAgentChatProfile {
    ResolvedAgentChatProfile {
        source: AgentChatProfileSource::BuiltIn,
        id: BUILTIN_GENERAL_PROFILE_ID.to_string(),
        name: "General".to_string(),
        icon_name: Some("sparkles".to_string()),
        backend: AgentChatBackend::Pi,
        pi_binary: None,
        agent: None,
        provider: Some(DEFAULT_PI_PROVIDER.to_string()),
        model: Some(DEFAULT_PI_MODEL.to_string()),
        system_prompt: None,
        append_system_prompt: Some(GENERAL_APPEND_SYSTEM_PROMPT.to_string()),
        cwd: Some(ctx.general_cwd()),
        tools: Some(
            GENERAL_PI_TOOLS
                .iter()
                .map(|tool| tool.to_string())
                .collect(),
        ),
        tool_policy: Some(AgentChatToolPolicyConfig {
            allow: Some(
                GENERAL_PI_TOOLS
                    .iter()
                    .map(|tool| tool.to_string())
                    .collect(),
            ),
        }),
        path_policy: Some(AgentChatPathPolicyConfig {
            allow_read: Some(vec![ctx.general_cwd().to_string_lossy().into_owned()]),
            allow_write: Some(vec![ctx.general_cwd().to_string_lossy().into_owned()]),
            deny: None,
        }),
        blocked_action_message: Some(GENERAL_BLOCKED_ACTION_MESSAGE.to_string()),
        disable_extensions: Some(true),
        disable_skills: Some(true),
        disable_prompt_templates: Some(true),
        disable_context_files: Some(true),
        hide_cwd_in_prompt: Some(true),
        thinking: Some(DEFAULT_PI_THINKING.to_string()),
        extension_policy: None,
        session_dir: None,
        no_session: Some(false),
        session_durability: None,
    }
}

pub fn built_in_script_kit_profile(ctx: &AgentChatProfileContext) -> ResolvedAgentChatProfile {
    ResolvedAgentChatProfile {
        source: AgentChatProfileSource::BuiltIn,
        id: BUILTIN_SCRIPT_KIT_PROFILE_ID.to_string(),
        name: "Script Kit".to_string(),
        icon_name: Some("code".to_string()),
        backend: AgentChatBackend::Pi,
        pi_binary: None,
        agent: None,
        provider: Some(DEFAULT_PI_PROVIDER.to_string()),
        model: Some(DEFAULT_PI_MODEL.to_string()),
        system_prompt: None,
        append_system_prompt: Some(SCRIPT_KIT_APPEND_SYSTEM_PROMPT.to_string()),
        cwd: Some(ctx.script_kit_cwd()),
        tools: Some(
            SCRIPT_KIT_PI_TOOLS
                .iter()
                .map(|tool| tool.to_string())
                .collect(),
        ),
        tool_policy: Some(AgentChatToolPolicyConfig {
            allow: Some(
                SCRIPT_KIT_PI_TOOLS
                    .iter()
                    .map(|tool| tool.to_string())
                    .collect(),
            ),
        }),
        path_policy: Some(AgentChatPathPolicyConfig {
            allow_read: Some(vec![ctx.script_kit_cwd().to_string_lossy().into_owned()]),
            allow_write: Some(vec![ctx.script_kit_cwd().to_string_lossy().into_owned()]),
            deny: None,
        }),
        blocked_action_message: None,
        disable_extensions: Some(true),
        disable_skills: Some(true),
        disable_prompt_templates: Some(true),
        disable_context_files: Some(true),
        hide_cwd_in_prompt: Some(false),
        thinking: Some(DEFAULT_PI_THINKING.to_string()),
        extension_policy: None,
        session_dir: None,
        no_session: Some(false),
        session_durability: None,
    }
}

pub fn built_in_text_profile(ctx: &AgentChatProfileContext) -> ResolvedAgentChatProfile {
    ResolvedAgentChatProfile {
        source: AgentChatProfileSource::BuiltIn,
        id: BUILTIN_TEXT_PROFILE_ID.to_string(),
        name: "Text".to_string(),
        icon_name: Some("file-text".to_string()),
        backend: AgentChatBackend::Pi,
        pi_binary: None,
        agent: None,
        provider: Some(DEFAULT_PI_PROVIDER.to_string()),
        model: Some(TEXT_PI_MODEL.to_string()),
        system_prompt: None,
        append_system_prompt: Some(TEXT_APPEND_SYSTEM_PROMPT.to_string()),
        cwd: Some(ctx.text_cwd()),
        tools: Some(TEXT_PI_TOOLS.iter().map(|tool| tool.to_string()).collect()),
        tool_policy: Some(AgentChatToolPolicyConfig {
            allow: Some(TEXT_PI_TOOLS.iter().map(|tool| tool.to_string()).collect()),
        }),
        path_policy: Some(AgentChatPathPolicyConfig {
            allow_read: Some(Vec::new()),
            allow_write: Some(Vec::new()),
            deny: None,
        }),
        blocked_action_message: Some(TEXT_BLOCKED_ACTION_MESSAGE.to_string()),
        disable_extensions: Some(true),
        disable_skills: Some(true),
        disable_prompt_templates: Some(true),
        disable_context_files: Some(true),
        hide_cwd_in_prompt: Some(true),
        thinking: None,
        extension_policy: Some("deny".to_string()),
        session_dir: None,
        no_session: Some(true),
        session_durability: None,
    }
}

/// Zero-context profile behind the launcher's Tab-with-text "Quick AI" mode.
///
/// Everything local is stripped: no extensions, no skills, no prompt
/// templates, no context files, no session persistence, and an empty path
/// policy. The single allowed tool is `web_search` ([`QUICK_AI_PI_TOOLS`])
/// so live-info questions get real answers — same posture as the Text
/// profile. The model is pinned to [`QUICK_AI_PI_MODEL`] and must
/// not be overridden by the user's Agent Chat model selection — resolve it
/// via `resolve_quick_ai_pi_launch`, never through `apply_ai_fallbacks`.
///
/// Intentionally NOT listed in [`built_in_profiles`]: Quick AI is a launch
/// mode, not a pickable profile.
pub fn built_in_quick_ai_profile(ctx: &AgentChatProfileContext) -> ResolvedAgentChatProfile {
    ResolvedAgentChatProfile {
        source: AgentChatProfileSource::BuiltIn,
        id: BUILTIN_QUICK_AI_PROFILE_ID.to_string(),
        name: "Quick AI".to_string(),
        icon_name: Some("zap".to_string()),
        backend: AgentChatBackend::Pi,
        pi_binary: None,
        agent: None,
        provider: Some(DEFAULT_PI_PROVIDER.to_string()),
        model: Some(QUICK_AI_PI_MODEL.to_string()),
        system_prompt: None,
        append_system_prompt: Some(QUICK_AI_APPEND_SYSTEM_PROMPT.to_string()),
        cwd: Some(ctx.quick_ai_cwd()),
        tools: Some(QUICK_AI_PI_TOOLS.iter().map(|s| s.to_string()).collect()),
        tool_policy: Some(AgentChatToolPolicyConfig {
            allow: Some(QUICK_AI_PI_TOOLS.iter().map(|s| s.to_string()).collect()),
        }),
        path_policy: Some(AgentChatPathPolicyConfig {
            allow_read: Some(Vec::new()),
            allow_write: Some(Vec::new()),
            deny: None,
        }),
        blocked_action_message: Some(QUICK_AI_BLOCKED_ACTION_MESSAGE.to_string()),
        disable_extensions: Some(true),
        disable_skills: Some(true),
        disable_prompt_templates: Some(true),
        disable_context_files: Some(true),
        hide_cwd_in_prompt: Some(true),
        thinking: None,
        extension_policy: Some("deny".to_string()),
        session_dir: None,
        no_session: Some(true),
        session_durability: None,
    }
}

pub fn built_in_profiles(ctx: &AgentChatProfileContext) -> Vec<ResolvedAgentChatProfile> {
    vec![
        built_in_brain_profile(ctx),
        built_in_general_profile(ctx),
        built_in_text_profile(ctx),
        built_in_script_kit_profile(ctx),
    ]
}

pub fn resolve_effective_profile(
    ai: &AiPreferences,
    ctx: &AgentChatProfileContext,
) -> ResolvedAgentChatProfile {
    let built_ins = built_in_profiles(ctx);
    let mdflow_profiles = resolved_mdflow_profiles(ctx);

    if let Some(selected_id) = clean_opt(ai.selected_profile_id.as_deref()) {
        if let Some(profile) = ai.profiles.iter().find(|profile| {
            clean_opt(profile.id.as_deref()) == Some(selected_id)
                || generated_legacy_profile_id(&profile.name) == selected_id
        }) {
            return apply_ai_fallbacks(resolve_user_profile(profile), ai);
        }

        if let Some(profile) = mdflow_profiles
            .iter()
            .find(|profile| profile.id == selected_id)
        {
            return apply_ai_fallbacks(profile.clone(), ai);
        }

        if let Some(profile) = built_ins.iter().find(|profile| profile.id == selected_id) {
            return apply_ai_fallbacks(profile.clone(), ai);
        }
    }

    if let Some(selected_name) = clean_opt(ai.selected_profile_name.as_deref()) {
        if let Some(profile) = ai
            .profiles
            .iter()
            .find(|profile| profile.name.trim() == selected_name)
        {
            return apply_ai_fallbacks(resolve_user_profile(profile), ai);
        }

        if let Some(profile) = mdflow_profiles
            .iter()
            .find(|profile| profile.name.eq_ignore_ascii_case(selected_name))
        {
            return apply_ai_fallbacks(profile.clone(), ai);
        }

        if let Some(profile) = built_ins
            .iter()
            .find(|profile| profile.name.eq_ignore_ascii_case(selected_name))
        {
            return apply_ai_fallbacks(profile.clone(), ai);
        }
    }

    // No explicit selection: the memory-aware Brain profile is the default —
    // quick questions typed into the launcher should land on the profile that
    // knows the user.
    apply_ai_fallbacks(built_in_brain_profile(ctx), ai)
}

/// Resolve the profile the launcher's Quick AI mode should launch with.
///
/// `ai.quick_ai_profile_id` (set from the Shift+Tab Profile Search via Tab =
/// "Use for Quick AI") picks any pickable profile; `None` or the built-in
/// `quick-ai` id keeps the pinned fast zero-context default. Picked profiles
/// go through `apply_ai_fallbacks` so they behave exactly as they do when
/// selected in Agent Chat; the default stays pinned (never fallback-mapped)
/// so the spark model can't be overridden by the global model selection.
pub fn resolve_quick_ai_profile(
    ai: &AiPreferences,
    ctx: &AgentChatProfileContext,
) -> ResolvedAgentChatProfile {
    if let Some(selected_id) = clean_opt(ai.quick_ai_profile_id.as_deref()) {
        if selected_id != BUILTIN_QUICK_AI_PROFILE_ID {
            if let Some(profile) = resolved_agent_chat_profile_picker_profiles(ai, ctx)
                .into_iter()
                .find(|profile| profile.id == selected_id)
            {
                return apply_ai_fallbacks(profile, ai);
            }
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "quick_ai_profile_selection_missing",
                profile_id = %selected_id,
                "Quick AI profile selection no longer resolves; using the built-in default"
            );
        }
    }
    built_in_quick_ai_profile(ctx)
}

/// Persist the profile Quick AI should use. Accepts any pickable profile id
/// plus the built-in `quick-ai` id (which restores the fast default).
/// Returns the resolved profile on success.
pub fn persist_quick_ai_profile_selection(
    ai: &mut AiPreferences,
    profile_id: &str,
    ctx: &AgentChatProfileContext,
) -> Option<ResolvedAgentChatProfile> {
    let profile = if profile_id == BUILTIN_QUICK_AI_PROFILE_ID {
        built_in_quick_ai_profile(ctx)
    } else {
        resolved_agent_chat_profile_picker_profiles(ai, ctx)
            .into_iter()
            .find(|profile| profile.id == profile_id)?
    };
    ai.quick_ai_profile_id = Some(profile.id.clone());
    Some(profile)
}

pub fn agent_chat_profile_picker_entries(
    ai: &AiPreferences,
    ctx: &AgentChatProfileContext,
) -> Vec<AgentChatProfilePickerEntry> {
    resolved_agent_chat_profile_picker_profiles(ai, ctx)
        .into_iter()
        .map(AgentChatProfilePickerEntry::from_profile)
        .collect()
}

pub fn resolved_agent_chat_profile_picker_profiles(
    ai: &AiPreferences,
    ctx: &AgentChatProfileContext,
) -> Vec<ResolvedAgentChatProfile> {
    let mut entries = built_in_profiles(ctx);

    for profile in resolved_mdflow_profiles(ctx) {
        if entries.iter().any(|entry| entry.id == profile.id) {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "agent_chat_profile_picker_duplicate_id_skipped",
                profile_id = %profile.id,
                profile_name = %profile.name,
            );
            continue;
        }
        entries.push(profile);
    }

    for profile in ai
        .profiles
        .iter()
        .filter(|profile| !profile.name.trim().is_empty())
        .map(resolve_user_profile)
    {
        if entries.iter().any(|entry| entry.id == profile.id) {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "agent_chat_profile_picker_duplicate_id_skipped",
                profile_id = %profile.id,
                profile_name = %profile.name,
            );
            continue;
        }
        entries.push(profile);
    }

    entries
}

pub fn selected_agent_chat_profile_picker_id(
    ai: &AiPreferences,
    ctx: &AgentChatProfileContext,
) -> String {
    resolve_effective_profile(ai, ctx).id
}

pub fn persist_agent_chat_profile_selection(
    ai: &mut AiPreferences,
    profile_id: &str,
    ctx: &AgentChatProfileContext,
) -> Option<AgentChatProfilePickerEntry> {
    let entries = agent_chat_profile_picker_entries(ai, ctx);
    let entry = entries
        .into_iter()
        .find(|entry| entry.id == profile_id)?
        .clone();

    ai.selected_profile_id = Some(entry.id.clone());
    ai.selected_profile_name = None;
    ai.selected_backend = Some(AgentChatBackend::Pi);
    Some(entry)
}

pub fn resolve_user_profile(profile: &AgentChatProfile) -> ResolvedAgentChatProfile {
    let backend = profile.backend.unwrap_or(AgentChatBackend::Pi);
    ResolvedAgentChatProfile {
        source: AgentChatProfileSource::User,
        id: profile
            .id
            .as_deref()
            .and_then(|id| clean_opt(Some(id)))
            .map(str::to_string)
            .unwrap_or_else(|| generated_legacy_profile_id(&profile.name)),
        name: profile.name.trim().to_string(),
        icon_name: clean_opt(profile.icon_name.as_deref()).map(str::to_string),
        backend,
        pi_binary: clean_opt(profile.pi_binary.as_deref())
            .map(crate::ai::agent_chat::pi::binary::expand_tilde_path),
        agent: clean_opt(profile.agent.as_deref()).map(str::to_string),
        provider: clean_opt(profile.provider.as_deref()).map(str::to_string),
        model: clean_opt(profile.model.as_deref()).map(str::to_string),
        system_prompt: clean_opt(profile.system_prompt.as_deref()).map(str::to_string),
        append_system_prompt: clean_opt(profile.append_system_prompt.as_deref())
            .map(str::to_string),
        cwd: clean_opt(profile.cwd.as_deref())
            .map(crate::ai::agent_chat::pi::binary::expand_tilde_path),
        tools: resolved_profile_tools(profile),
        tool_policy: profile.tool_policy.clone(),
        path_policy: profile.path_policy.clone(),
        blocked_action_message: clean_opt(profile.blocked_action_message.as_deref())
            .map(str::to_string),
        disable_extensions: profile.disable_extensions,
        disable_skills: profile.disable_skills,
        disable_prompt_templates: profile.disable_prompt_templates,
        disable_context_files: profile.disable_context_files,
        hide_cwd_in_prompt: profile.hide_cwd_in_prompt,
        thinking: clean_opt(profile.thinking.as_deref()).map(str::to_string),
        extension_policy: clean_opt(profile.extension_policy.as_deref()).map(str::to_string),
        session_dir: clean_opt(profile.session_dir.as_deref()).map(str::to_string),
        no_session: profile.no_session,
        session_durability: clean_opt(profile.session_durability.as_deref()).map(str::to_string),
    }
}

fn resolved_profile_tools(profile: &AgentChatProfile) -> Option<Vec<String>> {
    profile
        .tool_policy
        .as_ref()
        .and_then(|policy| policy.allow.as_ref())
        .map(|tools| clean_list(tools))
        .or_else(|| profile.tools.as_ref().map(|tools| clean_list(tools)))
}

pub fn apply_ai_fallbacks(
    mut profile: ResolvedAgentChatProfile,
    ai: &AiPreferences,
) -> ResolvedAgentChatProfile {
    if profile.pi_binary.is_none() {
        profile.pi_binary = clean_opt(ai.pi_binary.as_deref())
            .map(crate::ai::agent_chat::pi::binary::expand_tilde_path)
            .or_else(crate::ai::agent_chat::pi::binary::default_pi_binary);
    }

    if let Some(selected_model) = clean_opt(ai.selected_model_id.as_deref()) {
        if let Some((provider, model)) = parse_provider_model_selection(selected_model) {
            profile.provider = Some(provider);
            profile.model = Some(model);
        } else if profile.source == AgentChatProfileSource::BuiltIn || profile.model.is_none() {
            profile.model = Some(selected_model.to_string());
        }
    }

    if profile.thinking.is_none()
        && profile
            .model
            .as_deref()
            .is_some_and(|model| DEFAULT_MEDIUM_THINKING_MODELS.contains(&model.trim()))
    {
        profile.thinking = Some(DEFAULT_PI_THINKING.to_string());
    }

    profile
}

pub fn parse_provider_model_selection(raw: &str) -> Option<(String, String)> {
    let raw = clean_opt(Some(raw))?;
    let separator = raw.find('/').or_else(|| raw.find(':'))?;
    let provider = clean_opt(Some(&raw[..separator]))?;
    let model = clean_opt(Some(&raw[separator + 1..]))?;
    Some((provider.to_string(), model.to_string()))
}

pub fn clean_opt(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

pub fn clean_list(values: &[String]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| clean_opt(Some(value.as_str())).map(str::to_string))
        .collect()
}

pub fn generated_legacy_profile_id(name: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in name.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "legacy:profile".to_string()
    } else {
        format!("legacy:{slug}")
    }
}

#[cfg(test)]
mod built_in_profile_tests {
    use super::*;

    #[test]
    fn only_brain_profile_prompt_contract_mentions_brain_recall() {
        let ctx = AgentChatProfileContext {
            kit_path: std::path::PathBuf::from("/tmp/script-kit-profile-test"),
        };
        let brain = built_in_brain_profile(&ctx);
        let general = built_in_general_profile(&ctx);
        let script_kit = built_in_script_kit_profile(&ctx);
        let text = built_in_text_profile(&ctx);

        assert!(
            brain
                .append_system_prompt
                .as_deref()
                .is_some_and(|prompt| prompt.contains("Brain recall")),
            "Brain profile must describe the recall block it receives"
        );
        for profile in [general, script_kit, text] {
            assert!(
                !profile
                    .append_system_prompt
                    .as_deref()
                    .unwrap_or_default()
                    .contains("Brain recall"),
                "{} profile must not claim a Brain recall contract",
                profile.name
            );
        }
    }
}

#[cfg(test)]
mod quick_ai_profile_selection_tests {
    use super::*;

    fn ctx() -> AgentChatProfileContext {
        AgentChatProfileContext {
            kit_path: std::path::PathBuf::from("/tmp/script-kit-quick-ai-test"),
        }
    }

    #[test]
    fn quick_ai_defaults_to_built_in_when_unset() {
        let ai = AiPreferences::default();
        let profile = resolve_quick_ai_profile(&ai, &ctx());
        assert_eq!(profile.id, BUILTIN_QUICK_AI_PROFILE_ID);
        assert_eq!(profile.model.as_deref(), Some(QUICK_AI_PI_MODEL));
    }

    #[test]
    fn quick_ai_empty_search_fallback_is_bounded_and_truthful() {
        let prompt = QUICK_AI_APPEND_SYSTEM_PROMPT;

        assert!(prompt.contains("exactly one focused web_search call"));
        assert!(prompt.contains("Never call web_search again"));
        assert!(prompt.contains("completed search with no usable results is an empty result"));
        assert!(prompt.contains("do not claim you cannot access live news or sources"));
        assert!(prompt.contains("do not ask the user to supply a source"));
        assert!(prompt.contains("search returned no usable results"));
    }

    #[test]
    fn quick_ai_honors_picked_profile_id() {
        let ai = AiPreferences {
            quick_ai_profile_id: Some(BUILTIN_BRAIN_PROFILE_ID.to_string()),
            ..AiPreferences::default()
        };
        let profile = resolve_quick_ai_profile(&ai, &ctx());
        assert_eq!(profile.id, BUILTIN_BRAIN_PROFILE_ID);
    }

    #[test]
    fn quick_ai_falls_back_to_built_in_for_stale_selection() {
        let ai = AiPreferences {
            quick_ai_profile_id: Some("plugin:gone/never-existed".to_string()),
            ..AiPreferences::default()
        };
        let profile = resolve_quick_ai_profile(&ai, &ctx());
        assert_eq!(profile.id, BUILTIN_QUICK_AI_PROFILE_ID);
    }

    #[test]
    fn persist_accepts_quick_ai_id_and_picker_ids_only() {
        let ctx = ctx();
        let mut ai = AiPreferences::default();

        let restored =
            persist_quick_ai_profile_selection(&mut ai, BUILTIN_QUICK_AI_PROFILE_ID, &ctx);
        assert!(restored.is_some());
        assert_eq!(
            ai.quick_ai_profile_id.as_deref(),
            Some(BUILTIN_QUICK_AI_PROFILE_ID)
        );

        let picked = persist_quick_ai_profile_selection(&mut ai, BUILTIN_TEXT_PROFILE_ID, &ctx);
        assert!(picked.is_some());
        assert_eq!(
            ai.quick_ai_profile_id.as_deref(),
            Some(BUILTIN_TEXT_PROFILE_ID)
        );

        let rejected = persist_quick_ai_profile_selection(&mut ai, "no-such-profile", &ctx);
        assert!(rejected.is_none());
        // A rejected id must not clobber the previous valid selection.
        assert_eq!(
            ai.quick_ai_profile_id.as_deref(),
            Some(BUILTIN_TEXT_PROFILE_ID)
        );
    }

    #[test]
    fn quick_ai_selection_does_not_leak_into_agent_chat_default() {
        let ai = AiPreferences {
            quick_ai_profile_id: Some(BUILTIN_SCRIPT_KIT_PROFILE_ID.to_string()),
            ..AiPreferences::default()
        };
        // The Agent Chat default resolution must ignore the Quick AI pick.
        let effective = resolve_effective_profile(&ai, &ctx());
        assert_eq!(effective.id, BUILTIN_BRAIN_PROFILE_ID);
    }
}

//! Markdown Agent Chat profiles in the mdflow file format.
//!
//! One profile per `~/.scriptkit/profiles/<id>.md`: YAML frontmatter plus a
//! markdown body. The frontmatter keys are the same kebab-case flags the pi
//! CLI accepts (mdflow's passthrough convention — `model:`, `tools:`,
//! `thinking:`, `no-session:` …), so a profile file is also a valid
//! [mdflow](https://mdflow.dev) agent. The body becomes the profile's
//! instructions (`--append-system-prompt`).
//!
//! This replaces the retired `plugins/*/profiles/*/profile.json` pipeline:
//! creating a profile is now "drop one markdown file in `~/.scriptkit/profiles`"
//! (or press the Create action in the Shift+Tab Profile Search).

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::profiles::{AgentChatProfileContext, AgentChatProfileSource, ResolvedAgentChatProfile};
use crate::config::{AgentChatBackend, AgentChatToolPolicyConfig};

/// Directory holding markdown profiles: `<kit>/profiles`.
pub fn mdflow_profiles_dir(ctx: &AgentChatProfileContext) -> PathBuf {
    ctx.kit_path.join("profiles")
}

/// Frontmatter template used by the "Create New Profile" action. Kept in the
/// pi-flag passthrough shape so the file doubles as a runnable mdflow agent.
pub const MDFLOW_PROFILE_TEMPLATE: &str = r#"---
name: My Profile
model: openai-codex/gpt-5.3-codex-spark
tools: web_search
no-session: true
---

You are a focused Agent Chat profile. Describe the job, the tone, and the
boundaries here — this body is the profile's instructions.
"#;

const CACHE_TTL: Duration = Duration::from_secs(2);

struct MdflowProfileCacheEntry {
    dir: PathBuf,
    refreshed_at: Instant,
    profiles: Vec<ResolvedAgentChatProfile>,
}

static MDFLOW_PROFILE_CACHE: Mutex<Option<MdflowProfileCacheEntry>> = Mutex::new(None);

/// Drop the memoized profile list so the next lookup re-reads the directory
/// (used after the Create action writes a new file).
pub fn invalidate_mdflow_profile_cache() {
    if let Ok(mut cache) = MDFLOW_PROFILE_CACHE.lock() {
        *cache = None;
    }
}

/// Load all markdown profiles, memoized for a couple of seconds — Profile
/// Search and the composer picker resolve profiles several times per
/// keystroke and must not re-walk the directory each time.
pub fn resolved_mdflow_profiles(ctx: &AgentChatProfileContext) -> Vec<ResolvedAgentChatProfile> {
    let dir = mdflow_profiles_dir(ctx);
    let now = Instant::now();
    if let Ok(cache) = MDFLOW_PROFILE_CACHE.lock() {
        if let Some(entry) = cache.as_ref() {
            if entry.dir == dir && now.duration_since(entry.refreshed_at) < CACHE_TTL {
                return entry.profiles.clone();
            }
        }
    }

    let profiles = resolved_mdflow_profiles_uncached(&dir);
    if let Ok(mut cache) = MDFLOW_PROFILE_CACHE.lock() {
        *cache = Some(MdflowProfileCacheEntry {
            dir,
            refreshed_at: now,
            profiles: profiles.clone(),
        });
    }
    profiles
}

fn resolved_mdflow_profiles_uncached(dir: &PathBuf) -> Vec<ResolvedAgentChatProfile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut profiles: Vec<ResolvedAgentChatProfile> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                return None;
            }
            let stem = path.file_stem()?.to_str()?.to_string();
            let content = match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(error) => {
                    tracing::warn!(
                        target: "script_kit::agent_chat",
                        event = "mdflow_profile_read_failed",
                        path = %path.display(),
                        %error,
                    );
                    return None;
                }
            };
            match parse_mdflow_profile(&stem, &content) {
                Ok(profile) => Some(profile),
                Err(error) => {
                    tracing::warn!(
                        target: "script_kit::agent_chat",
                        event = "mdflow_profile_parse_failed",
                        path = %path.display(),
                        %error,
                    );
                    None
                }
            }
        })
        .collect();
    profiles.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    profiles
}

/// Parse one markdown profile. `stem` (the filename without `.md`) is the
/// profile id. Unknown frontmatter keys are ignored — they may be mdflow
/// engine flags that only matter when the file is run with `md`.
pub fn parse_mdflow_profile(stem: &str, content: &str) -> Result<ResolvedAgentChatProfile, String> {
    let (frontmatter, body) = split_frontmatter(content)?;
    let mapping: serde_yaml::Value = if frontmatter.trim().is_empty() {
        serde_yaml::Value::Mapping(Default::default())
    } else {
        serde_yaml::from_str(frontmatter).map_err(|error| error.to_string())?
    };
    let mapping = mapping
        .as_mapping()
        .ok_or_else(|| "frontmatter must be a YAML mapping".to_string())?;

    let name = optional_string(mapping, "name")?.unwrap_or_else(|| title_case_stem(stem));
    let raw_model = optional_string(mapping, "model")?;
    let explicit_provider = optional_string(mapping, "provider")?;
    let icon_name = optional_string(mapping, "icon")?;
    let system_prompt = optional_string(mapping, "system-prompt")?;
    let cwd = optional_string(mapping, "cwd")?;
    let fallback_cwd = optional_string(mapping, "_cwd")?;
    let thinking = optional_string(mapping, "thinking")?;
    let tools = optional_string_list(mapping, "tools")?;
    let disable_extensions = optional_bool(mapping, "no-extensions")?;
    let disable_skills = optional_bool(mapping, "no-skills")?;
    let disable_prompt_templates = optional_bool(mapping, "no-prompt-templates")?;
    let disable_context_files = optional_bool(mapping, "no-context-files")?;
    let no_session = optional_bool(mapping, "no-session")?;

    // `model` accepts mdflow/pi's "provider/id" shorthand; an explicit
    // `provider:` key wins over the shorthand prefix.
    let (shorthand_provider, model) = match raw_model {
        Some(raw) => match raw.split_once('/') {
            Some((provider, model)) if !provider.trim().is_empty() && !model.trim().is_empty() => (
                Some(provider.trim().to_string()),
                Some(model.trim().to_string()),
            ),
            _ => (None, Some(raw)),
        },
        None => (None, None),
    };
    let provider = explicit_provider.or(shorthand_provider);

    let body = body.trim();
    let append_system_prompt = if body.is_empty() {
        None
    } else {
        Some(body.to_string())
    };

    Ok(ResolvedAgentChatProfile {
        source: AgentChatProfileSource::Mdflow,
        id: stem.to_string(),
        name,
        icon_name,
        backend: AgentChatBackend::Pi,
        pi_binary: None,
        agent: None,
        provider,
        model,
        system_prompt,
        append_system_prompt,
        cwd: cwd
            .or(fallback_cwd)
            .map(|value| crate::ai::agent_chat::pi::binary::expand_tilde_path(&value)),
        tool_policy: tools.as_ref().map(|tools| AgentChatToolPolicyConfig {
            allow: Some(tools.clone()),
        }),
        tools,
        path_policy: None,
        blocked_action_message: None,
        disable_extensions,
        disable_skills,
        disable_prompt_templates,
        disable_context_files,
        hide_cwd_in_prompt: None,
        thinking,
        extension_policy: None,
        session_dir: None,
        no_session,
        session_durability: None,
    })
}

/// Split `---` frontmatter from the markdown body. Files without frontmatter
/// are all body (instructions-only profiles are valid).
fn split_frontmatter(content: &str) -> Result<(&str, &str), String> {
    let trimmed = content.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let Some(rest) = trimmed.strip_prefix("---") else {
        return Ok(("", trimmed));
    };
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))
        .ok_or_else(|| "frontmatter opening `---` must be on its own line".to_string())?;
    for (offset, _) in rest.match_indices("\n---") {
        let after = &rest[offset + 4..];
        let after_line_end = after
            .strip_prefix('\n')
            .or_else(|| after.strip_prefix("\r\n"))
            .or(if after.is_empty() { Some("") } else { None });
        if let Some(body) = after_line_end {
            return Ok((&rest[..offset], body));
        }
    }
    Err("frontmatter is missing its closing `---`".to_string())
}

fn optional_string(mapping: &serde_yaml::Mapping, key: &str) -> Result<Option<String>, String> {
    let Some(value) = mapping.get(serde_yaml::Value::String(key.to_string())) else {
        return Ok(None);
    };
    match value {
        serde_yaml::Value::String(text) => {
            Ok(Some(text.clone()).filter(|text| !text.trim().is_empty()))
        }
        _ => Err(format!("frontmatter key `{key}` must be a YAML string")),
    }
}

fn optional_bool(mapping: &serde_yaml::Mapping, key: &str) -> Result<Option<bool>, String> {
    let Some(value) = mapping.get(serde_yaml::Value::String(key.to_string())) else {
        return Ok(None);
    };
    match value {
        serde_yaml::Value::Bool(flag) => Ok(Some(*flag)),
        _ => Err(format!("frontmatter key `{key}` must be a YAML boolean")),
    }
}

/// Tools accept both a YAML list of strings and pi's comma-separated string form.
fn optional_string_list(
    mapping: &serde_yaml::Mapping,
    key: &str,
) -> Result<Option<Vec<String>>, String> {
    let Some(value) = mapping.get(serde_yaml::Value::String(key.to_string())) else {
        return Ok(None);
    };
    let invalid_type =
        || format!("frontmatter key `{key}` must be a YAML string or list of strings");
    let values = match value {
        serde_yaml::Value::Sequence(items) => items
            .iter()
            .map(|item| match item {
                serde_yaml::Value::String(text) => Ok(text.trim().to_string()),
                _ => Err(invalid_type()),
            })
            .collect::<Result<Vec<_>, _>>()?,
        serde_yaml::Value::String(text) => text
            .split(',')
            .map(|item| item.trim().to_string())
            .collect(),
        _ => return Err(invalid_type()),
    };
    Ok(Some(
        values.into_iter().filter(|item| !item.is_empty()).collect(),
    ))
}

fn title_case_stem(stem: &str) -> String {
    stem.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Create a new profile file from [`MDFLOW_PROFILE_TEMPLATE`], picking a
/// filename that does not collide with an existing profile. Returns the path.
pub fn create_mdflow_profile_from_template(
    ctx: &AgentChatProfileContext,
) -> std::io::Result<PathBuf> {
    let dir = mdflow_profiles_dir(ctx);
    std::fs::create_dir_all(&dir)?;
    let mut path = dir.join("my-profile.md");
    let mut counter = 2u32;
    while path.exists() {
        path = dir.join(format!("my-profile-{counter}.md"));
        counter += 1;
    }
    std::fs::write(&path, MDFLOW_PROFILE_TEMPLATE)?;
    invalidate_mdflow_profile_cache();
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_frontmatter_and_body() {
        let profile = parse_mdflow_profile(
            "docs-researcher",
            "---\nname: Docs Researcher\nmodel: openai-codex/gpt-5.6-terra\ntools: web_search, read\nthinking: medium\nno-session: true\ncwd: ~/notes\n---\n\nResearch docs and cite sources.\n",
        )
        .expect("profile parses");

        assert_eq!(profile.id, "docs-researcher");
        assert_eq!(profile.name, "Docs Researcher");
        assert_eq!(profile.source, AgentChatProfileSource::Mdflow);
        assert_eq!(profile.provider.as_deref(), Some("openai-codex"));
        assert_eq!(profile.model.as_deref(), Some("gpt-5.6-terra"));
        assert_eq!(
            profile.tools,
            Some(vec!["web_search".to_string(), "read".to_string()])
        );
        assert_eq!(profile.thinking.as_deref(), Some("medium"));
        assert_eq!(profile.no_session, Some(true));
        assert!(profile
            .cwd
            .as_ref()
            .is_some_and(|cwd| cwd.ends_with("notes")));
        assert_eq!(
            profile.append_system_prompt.as_deref(),
            Some("Research docs and cite sources.")
        );
    }

    #[test]
    fn tools_accept_yaml_list_form() {
        let profile = parse_mdflow_profile(
            "lister",
            "---\ntools:\n  - web_search\n  - grep\n---\nBody.\n",
        )
        .expect("profile parses");
        assert_eq!(
            profile.tools,
            Some(vec!["web_search".to_string(), "grep".to_string()])
        );
        assert_eq!(
            profile
                .tool_policy
                .and_then(|policy| policy.allow)
                .unwrap_or_default()
                .len(),
            2
        );
    }

    #[test]
    fn body_only_file_is_an_instructions_profile_named_from_stem() {
        let profile = parse_mdflow_profile("code-review-buddy", "Be a strict reviewer.\n")
            .expect("profile parses");
        assert_eq!(profile.name, "Code Review Buddy");
        assert_eq!(profile.model, None);
        assert_eq!(
            profile.append_system_prompt.as_deref(),
            Some("Be a strict reviewer.")
        );
    }

    #[test]
    fn explicit_provider_key_beats_model_shorthand() {
        let profile = parse_mdflow_profile(
            "p",
            "---\nprovider: google-antigravity\nmodel: openai-codex/gpt-5.4\n---\n",
        )
        .expect("profile parses");
        assert_eq!(profile.provider.as_deref(), Some("google-antigravity"));
        assert_eq!(profile.model.as_deref(), Some("gpt-5.4"));
    }

    #[test]
    fn wrong_typed_name_mapping_invalidates_profile() {
        let error = parse_mdflow_profile("bad-name", "---\nname:\n  nested: value\n---\nBody.\n")
            .expect_err("a present mapping-valued name must invalidate the profile");
        assert!(error.contains("name"), "unexpected error: {error}");
        assert!(error.contains("string"), "unexpected error: {error}");
    }

    #[test]
    fn wrong_typed_model_mapping_invalidates_profile() {
        let error =
            parse_mdflow_profile("bad-model", "---\nmodel:\n  provider: openai\n---\nBody.\n")
                .expect_err("a present mapping-valued model must invalidate the profile");
        assert!(error.contains("model"), "unexpected error: {error}");
        assert!(error.contains("string"), "unexpected error: {error}");
    }

    #[test]
    fn wrong_typed_tool_item_mapping_invalidates_profile() {
        let error = parse_mdflow_profile(
            "bad-tool",
            "---\ntools:\n  - web_search\n  - nested: value\n---\nBody.\n",
        )
        .expect_err("a present mapping-valued tool item must invalidate the profile");
        assert!(error.contains("tools"), "unexpected error: {error}");
        assert!(
            error.contains("string or list of strings"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn wrong_typed_no_session_string_invalidates_profile() {
        let error =
            parse_mdflow_profile("bad-no-session", "---\nno-session: \"true\"\n---\nBody.\n")
                .expect_err("a present string-valued no-session must invalidate the profile");
        assert!(error.contains("no-session"), "unexpected error: {error}");
        assert!(error.contains("boolean"), "unexpected error: {error}");
    }

    #[test]
    fn unclosed_frontmatter_is_an_error_not_a_silent_profile() {
        assert!(parse_mdflow_profile("bad", "---\nname: Broken\n").is_err());
    }

    #[test]
    fn template_parses_into_a_valid_profile() {
        let profile =
            parse_mdflow_profile("my-profile", MDFLOW_PROFILE_TEMPLATE).expect("template parses");
        assert_eq!(profile.name, "My Profile");
        assert_eq!(profile.tools, Some(vec!["web_search".to_string()]));
        assert_eq!(profile.no_session, Some(true));
        assert!(profile.append_system_prompt.is_some());
    }
}

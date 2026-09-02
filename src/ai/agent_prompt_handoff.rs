use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ai::message_parts::{AiContextPart, PreparedMessageDecision};
use crate::config::PromptTargetConfig;
use crate::spine::prompt_plan::{SpinePromptPlan, SpinePromptPlanBlockReason};

pub(crate) const PROMPT_TARGET_ACTION_PREFIX: &str = "prompt-target/";
pub(crate) const PROMPT_ACTION_PREFIX: &str = "prompt-action/";
pub(crate) const AGENT_PROMPT_HANDOFF_ACTION_PREFIX: &str = PROMPT_TARGET_ACTION_PREFIX;
pub(crate) const LEGACY_AGENT_PROMPT_HANDOFF_ACTION_PREFIX: &str = "agent_chat:handoff:";
pub(crate) const CMUX_CODEX_ADAPTER_ID: &str = "cmux_codex";
pub(crate) const CMUX_CODEX_TARGET_ID: &str = "cmux-codex";
pub(crate) const CMUX_CODEX_ACTION_ID: &str = "prompt-target/cmux-codex";
pub(crate) const LEGACY_CMUX_CODEX_ACTION_ID: &str = "agent_chat:handoff:cmux_codex";
pub(crate) const EXPORT_FILE_PROMPT_ACTION_ID: &str = "export-file";
pub(crate) const EXPORT_FILE_ACTION_ID: &str = "prompt-action/export-file";
pub(crate) const EXPORT_GIST_PROMPT_ACTION_ID: &str = "export-gist";
pub(crate) const EXPORT_GIST_ACTION_ID: &str = "prompt-action/export-gist";
pub(crate) const COPY_PROMPT_PROMPT_ACTION_ID: &str = "copy-prompt";
pub(crate) const COPY_PROMPT_ACTION_ID: &str = "prompt-action/copy-prompt";

const DRY_RUN_ENV: &str = "SCRIPT_KIT_AGENT_HANDOFF_DRY_RUN";
const RECEIPT_PATH_ENV: &str = "SCRIPT_KIT_AGENT_HANDOFF_RECEIPT_PATH";
const CMUX_BINARY_ENV: &str = "SCRIPT_KIT_CMUX_BINARY";
const CODEX_BINARY_ENV: &str = "SCRIPT_KIT_CODEX_BINARY";
const PROMPT_EXPORT_DIR_ENV: &str = "SCRIPT_KIT_PROMPT_EXPORT_DIR";
const GH_BINARY_ENV: &str = "SCRIPT_KIT_GH_BINARY";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentPromptCommandTarget {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) env: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentPromptHandoffAdapterId {
    CmuxCodex,
    Command(AgentPromptCommandTarget),
}

impl AgentPromptHandoffAdapterId {
    pub(crate) fn id(&self) -> String {
        match self {
            Self::CmuxCodex => CMUX_CODEX_ADAPTER_ID.to_string(),
            Self::Command(target) => target.id.clone(),
        }
    }

    pub(crate) fn action_id(&self) -> String {
        match self {
            Self::CmuxCodex => CMUX_CODEX_ACTION_ID.to_string(),
            Self::Command(target) => prompt_target_action_id(&target.id),
        }
    }

    pub(crate) fn title(&self) -> &str {
        match self {
            Self::CmuxCodex => "cmux Codex",
            Self::Command(target) => &target.title,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentPromptHandoffSource {
    AgentChatComposer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentPromptHandoffPayload {
    pub(crate) source: AgentPromptHandoffSource,
    pub(crate) adapter_id: AgentPromptHandoffAdapterId,
    pub(crate) raw_input: String,
    pub(crate) prompt: String,
    pub(crate) cwd: PathBuf,
    pub(crate) model_id: Option<String>,
    pub(crate) profile_id: Option<String>,
    pub(crate) context_part_count: usize,
    pub(crate) prompt_builder_segment_count: usize,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentPromptActionId {
    ExportFile,
    ExportGist,
    CopyPrompt,
}

impl AgentPromptActionId {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::ExportFile => EXPORT_FILE_PROMPT_ACTION_ID,
            Self::ExportGist => EXPORT_GIST_PROMPT_ACTION_ID,
            Self::CopyPrompt => COPY_PROMPT_PROMPT_ACTION_ID,
        }
    }

    pub(crate) fn action_id(self) -> &'static str {
        match self {
            Self::ExportFile => EXPORT_FILE_ACTION_ID,
            Self::ExportGist => EXPORT_GIST_ACTION_ID,
            Self::CopyPrompt => COPY_PROMPT_ACTION_ID,
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::ExportFile => "Export Prompt to File",
            Self::ExportGist => "Export Prompt to Gist",
            Self::CopyPrompt => "Copy Prompt to Clipboard",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::ExportFile => "Save the current built prompt as a markdown file",
            Self::ExportGist => "Publish the current built prompt as a private GitHub gist",
            Self::CopyPrompt => "Copy the current built prompt to the clipboard",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentPromptHandoffReceipt {
    pub(crate) adapter_id: String,
    pub(crate) action_id: String,
    pub(crate) dry_run: bool,
    pub(crate) cwd: String,
    pub(crate) prompt_chars: usize,
    pub(crate) prompt_sha256: String,
    pub(crate) command_kind: String,
    pub(crate) cmux_binary: String,
    pub(crate) codex_binary: String,
    pub(crate) prompt_file_created: bool,
    pub(crate) script_file_created: bool,
    pub(crate) spawned: bool,
    pub(crate) pid: Option<u32>,
}

impl AgentPromptHandoffReceipt {
    /// The exact prompt digest remains a private interoperability receipt,
    /// but ordinary app diagnostics must never expose a guessable raw SHA.
    pub(crate) fn diagnostic_prompt_fingerprint(&self) -> String {
        crate::logging::log_private_user_value(&self.prompt_sha256).sha256
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentPromptExportReceipt {
    pub(crate) action_id: String,
    pub(crate) dry_run: bool,
    pub(crate) cwd: String,
    pub(crate) prompt_chars: usize,
    pub(crate) prompt_sha256: String,
    pub(crate) context_part_count: usize,
    pub(crate) prompt_builder_segment_count: usize,
    pub(crate) export_kind: String,
    pub(crate) path: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) command_kind: String,
    pub(crate) clipboard_written: bool,
    pub(crate) spawned: bool,
}

impl AgentPromptExportReceipt {
    pub(crate) fn diagnostic_prompt_fingerprint(&self) -> String {
        crate::logging::log_private_user_value(&self.prompt_sha256).sha256
    }

    pub(crate) fn diagnostic_path_fingerprint(&self) -> Option<String> {
        self.path
            .as_deref()
            .map(|path| crate::logging::log_private_user_value(path).sha256)
    }

    pub(crate) fn diagnostic_url_fingerprint(&self) -> Option<String> {
        self.url
            .as_deref()
            .map(|url| crate::logging::log_private_user_value(url).sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentPromptHandoffError {
    SetupMode,
    EmptyPrompt,
    UnsupportedPrompt(String),
    UnsupportedAdapter(String),
    Io(String),
    Spawn(String),
}

pub(crate) fn prompt_target_action_id(target_id: &str) -> String {
    format!("{PROMPT_TARGET_ACTION_PREFIX}{target_id}")
}

pub(crate) fn prompt_action_id(action_id: &str) -> String {
    format!("{PROMPT_ACTION_PREFIX}{action_id}")
}

pub(crate) fn builtin_prompt_targets() -> Vec<AgentPromptHandoffAdapterId> {
    vec![AgentPromptHandoffAdapterId::CmuxCodex]
}

pub(crate) fn builtin_prompt_actions() -> Vec<AgentPromptActionId> {
    vec![
        AgentPromptActionId::ExportFile,
        AgentPromptActionId::ExportGist,
        AgentPromptActionId::CopyPrompt,
    ]
}

pub(crate) fn configured_prompt_targets(
    config: &crate::config::Config,
) -> Vec<AgentPromptHandoffAdapterId> {
    let mut targets: Vec<_> = config
        .prompt_targets
        .as_ref()
        .into_iter()
        .flat_map(|targets| targets.iter())
        .filter_map(|(id, target)| command_target_from_config(id, target))
        .map(AgentPromptHandoffAdapterId::Command)
        .collect();
    targets.sort_by(|a, b| a.title().cmp(b.title()));
    targets
}

pub(crate) fn all_prompt_targets(
    config: &crate::config::Config,
) -> Vec<AgentPromptHandoffAdapterId> {
    let mut targets = builtin_prompt_targets();
    targets.extend(configured_prompt_targets(config));
    targets
}

fn command_target_from_config(
    id: &str,
    target: &PromptTargetConfig,
) -> Option<AgentPromptCommandTarget> {
    let normalized_id = id.trim();
    let command = target.command.trim();
    if normalized_id.is_empty() || command.is_empty() {
        return None;
    }

    Some(AgentPromptCommandTarget {
        id: normalized_id.to_string(),
        title: target
            .title
            .as_deref()
            .filter(|title: &&str| !title.trim().is_empty())
            .unwrap_or(normalized_id)
            .to_string(),
        description: target.description.clone(),
        command: command.to_string(),
        args: target.args.clone(),
        cwd: target
            .cwd
            .as_ref()
            .map(|cwd| PathBuf::from(shellexpand::tilde(cwd).to_string())),
        env: target.env.clone(),
    })
}

impl AgentPromptHandoffError {
    pub(crate) fn user_message(&self) -> String {
        match self {
            Self::SetupMode => "Agent Chat is in setup mode".to_string(),
            Self::EmptyPrompt => "No prompt to send".to_string(),
            Self::UnsupportedPrompt(reason) => format!("Prompt cannot be handed off: {reason}"),
            Self::UnsupportedAdapter(adapter) => {
                format!("Prompt handoff adapter '{adapter}' is unavailable")
            }
            Self::Io(error) => format!("Failed to prepare prompt handoff: {error}"),
            Self::Spawn(error) => format!("Failed to run prompt action: {error}"),
        }
    }
}

pub(crate) fn adapter_from_action_id(action_id: &str) -> Option<AgentPromptHandoffAdapterId> {
    match action_id {
        CMUX_CODEX_ACTION_ID | LEGACY_CMUX_CODEX_ACTION_ID => {
            Some(AgentPromptHandoffAdapterId::CmuxCodex)
        }
        _ => {
            let target_id = action_id.strip_prefix(PROMPT_TARGET_ACTION_PREFIX)?;
            let config = crate::config::load_config();
            configured_prompt_targets(&config)
                .into_iter()
                .find(|target| target.id() == target_id)
        }
    }
}

pub(crate) fn prompt_action_from_action_id(action_id: &str) -> Option<AgentPromptActionId> {
    let id = action_id.strip_prefix(PROMPT_ACTION_PREFIX)?;
    match id {
        EXPORT_FILE_PROMPT_ACTION_ID => Some(AgentPromptActionId::ExportFile),
        EXPORT_GIST_PROMPT_ACTION_ID => Some(AgentPromptActionId::ExportGist),
        COPY_PROMPT_PROMPT_ACTION_ID => Some(AgentPromptActionId::CopyPrompt),
        _ => None,
    }
}

pub(crate) fn is_prompt_action_id(action_id: &str) -> bool {
    adapter_from_action_id(action_id).is_some() || prompt_action_from_action_id(action_id).is_some()
}

pub(crate) fn launch_prompt_handoff(
    payload: &AgentPromptHandoffPayload,
) -> Result<AgentPromptHandoffReceipt, AgentPromptHandoffError> {
    if payload.prompt.trim().is_empty() {
        return Err(AgentPromptHandoffError::EmptyPrompt);
    }

    match &payload.adapter_id {
        AgentPromptHandoffAdapterId::CmuxCodex => launch_cmux_codex(payload),
        AgentPromptHandoffAdapterId::Command(ref target) => launch_command_target(payload, target),
    }
}

pub(crate) fn export_prompt(
    payload: &AgentPromptHandoffPayload,
    action: AgentPromptActionId,
) -> Result<AgentPromptExportReceipt, AgentPromptHandoffError> {
    if payload.prompt.trim().is_empty() {
        return Err(AgentPromptHandoffError::EmptyPrompt);
    }

    match action {
        AgentPromptActionId::ExportFile => export_prompt_to_file(payload, action),
        AgentPromptActionId::ExportGist => export_prompt_to_gist(payload, action),
        AgentPromptActionId::CopyPrompt => copy_prompt_to_clipboard(payload, action),
    }
}

pub(crate) fn compile_handoff_payload_from_spine_plan(
    adapter_id: AgentPromptHandoffAdapterId,
    raw_input: String,
    cwd: PathBuf,
    model_id: Option<String>,
    attached_parts: Vec<AiContextPart>,
    plan: SpinePromptPlan,
) -> Result<AgentPromptHandoffPayload, AgentPromptHandoffError> {
    if raw_input.trim().is_empty() {
        return Err(AgentPromptHandoffError::EmptyPrompt);
    }

    if plan.blocked_reason.is_some()
        && plan.blocked_reason != Some(SpinePromptPlanBlockReason::NoPromptBuilderSegments)
    {
        let reason = plan
            .blocked_reason
            .map(|reason| format!("{reason:?}"))
            .unwrap_or_else(|| "blocked prompt builder input".to_string());
        return Err(AgentPromptHandoffError::UnsupportedPrompt(format!(
            "prompt builder input is not submittable: {reason}"
        )));
    }

    if plan.prompt_builder_segment_count > 0 && !plan.should_submit_to_chat() {
        let reason = plan
            .blocked_reason
            .map(|reason| format!("{reason:?}"))
            .unwrap_or_else(|| "incomplete prompt builder input".to_string());
        return Err(AgentPromptHandoffError::UnsupportedPrompt(format!(
            "prompt builder input is not submittable: {reason}"
        )));
    }

    let mut context_parts = Vec::with_capacity(attached_parts.len() + plan.context_parts.len());
    for part in attached_parts.iter().chain(plan.context_parts.iter()) {
        if !context_parts.iter().any(|existing| existing == part) {
            context_parts.push(part.clone());
        }
    }

    let normalized_prompt = if plan.prompt_builder_segment_count > 0 {
        plan.normalized_prompt.trim().to_string()
    } else {
        raw_input.trim().to_string()
    };

    if normalized_prompt.is_empty() && context_parts.is_empty() {
        return Err(AgentPromptHandoffError::EmptyPrompt);
    }

    let scripts: Vec<std::sync::Arc<crate::scripts::Script>> = Vec::new();
    let scriptlets: Vec<std::sync::Arc<crate::scripts::Scriptlet>> = Vec::new();
    let preparation_items = context_parts
        .iter()
        .cloned()
        .map(crate::ai::message_parts::ContextPreparationItem::primary)
        .collect::<Vec<_>>();
    let prepared = crate::ai::message_parts::prepare_user_message(
        &normalized_prompt,
        &preparation_items,
        &scripts,
        &scriptlets,
    );
    if prepared.decision == PreparedMessageDecision::Blocked {
        return Err(AgentPromptHandoffError::UnsupportedPrompt(
            prepared
                .receipt
                .user_error
                .clone()
                .unwrap_or_else(|| "context preparation was blocked".to_string()),
        ));
    }

    let prompt = prepared.final_user_content.trim().to_string();
    if prompt.is_empty() {
        return Err(AgentPromptHandoffError::EmptyPrompt);
    }

    Ok(AgentPromptHandoffPayload {
        source: AgentPromptHandoffSource::AgentChatComposer,
        adapter_id,
        raw_input,
        prompt,
        cwd,
        model_id,
        profile_id: plan.selected_profile.map(|profile| profile.id),
        context_part_count: context_parts.len(),
        prompt_builder_segment_count: plan.prompt_builder_segment_count,
        warnings: plan
            .unknown_warnings
            .into_iter()
            .map(|warning| warning.preflight_instruction)
            .collect(),
    })
}

fn launch_cmux_codex(
    payload: &AgentPromptHandoffPayload,
) -> Result<AgentPromptHandoffReceipt, AgentPromptHandoffError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)
        .map_err(|error| AgentPromptHandoffError::Spawn(error.to_string()))?;
    if payload.prompt.contains('\0') {
        return Err(AgentPromptHandoffError::UnsupportedPrompt(
            "NUL bytes cannot be passed to Codex argv".to_string(),
        ));
    }

    let cmux_binary = std::env::var(CMUX_BINARY_ENV).unwrap_or_else(|_| "cmux".to_string());
    let codex_binary = std::env::var(CODEX_BINARY_ENV).unwrap_or_else(|_| "codex".to_string());
    let dry_run = env_truthy(DRY_RUN_ENV);

    let mut prompt_file_created = false;
    let mut script_file_created = false;
    let mut command_string = String::new();

    if !dry_run {
        let prepared = prepare_cmux_codex_wrapper(&payload.prompt, &codex_binary, &payload.cwd)?;
        prompt_file_created = true;
        script_file_created = true;
        command_string = prepared.command_string;
    }

    let prompt_chars = payload.prompt.chars().count();
    let prompt_sha256 = sha256_hex(&payload.prompt);
    let mut receipt = AgentPromptHandoffReceipt {
        adapter_id: payload.adapter_id.id().to_string(),
        action_id: payload.adapter_id.action_id().to_string(),
        dry_run,
        cwd: payload.cwd.to_string_lossy().to_string(),
        prompt_chars,
        prompt_sha256,
        command_kind: "cmux_workspace_surface_create_initial_command".to_string(),
        cmux_binary: cmux_binary.clone(),
        codex_binary,
        prompt_file_created,
        script_file_created,
        spawned: false,
        pid: None,
    };

    if dry_run {
        write_receipt_if_requested(&receipt)?;
        return Ok(receipt);
    }

    let workspace_args = build_cmux_workspace_create_rpc_args(&payload.cwd)?;
    let workspace_output = std::process::Command::new(&cmux_binary)
        .args(workspace_args)
        .output()
        .map_err(|error| AgentPromptHandoffError::Spawn(error.to_string()))?;
    if !workspace_output.status.success() {
        return Err(AgentPromptHandoffError::Spawn(
            String::from_utf8_lossy(&workspace_output.stderr)
                .trim()
                .to_string(),
        ));
    }
    let workspace_ref = parse_cmux_workspace_ref(&workspace_output.stdout)?;
    let surface_args =
        build_cmux_surface_create_rpc_args(&workspace_ref, &payload.cwd, &command_string)?;
    let child = std::process::Command::new(&cmux_binary)
        .args(surface_args)
        .spawn()
        .map_err(|error| AgentPromptHandoffError::Spawn(error.to_string()))?;

    receipt.spawned = true;
    receipt.pid = Some(child.id());
    write_receipt_if_requested(&receipt)?;
    Ok(receipt)
}

fn launch_command_target(
    payload: &AgentPromptHandoffPayload,
    target: &AgentPromptCommandTarget,
) -> Result<AgentPromptHandoffReceipt, AgentPromptHandoffError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)
        .map_err(|error| AgentPromptHandoffError::Spawn(error.to_string()))?;
    let dry_run = env_truthy(DRY_RUN_ENV);
    let cwd = target.cwd.clone().unwrap_or_else(|| payload.cwd.clone());
    let prompt_chars = payload.prompt.chars().count();
    let prompt_sha256 = sha256_hex(&payload.prompt);
    let mut prompt_file_created = false;
    let mut prompt_file_path = None;

    let needs_prompt_file = target.args.iter().any(|arg| arg.contains("{promptFile}"))
        || target
            .env
            .values()
            .any(|value| value.contains("{promptFile}"));
    if needs_prompt_file && !dry_run {
        let dir = create_private_handoff_directory(
            &std::env::temp_dir().join("script-kit-agent-handoff"),
        )?;
        let path = dir.join("prompt.md");
        write_private_handoff_file(&path, payload.prompt.as_bytes())?;
        prompt_file_created = true;
        prompt_file_path = Some(path);
    }

    let prompt_file = prompt_file_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let args = target
        .args
        .iter()
        .map(|arg| replace_prompt_placeholders(arg, &payload.prompt, &prompt_file))
        .collect::<Vec<_>>();
    let env = target
        .env
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                replace_prompt_placeholders(value, &payload.prompt, &prompt_file),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut receipt = AgentPromptHandoffReceipt {
        adapter_id: payload.adapter_id.id(),
        action_id: payload.adapter_id.action_id(),
        dry_run,
        cwd: cwd.to_string_lossy().to_string(),
        prompt_chars,
        prompt_sha256,
        command_kind: "prompt_target_command".to_string(),
        cmux_binary: String::new(),
        codex_binary: target.command.clone(),
        prompt_file_created,
        script_file_created: false,
        spawned: false,
        pid: None,
    };

    if dry_run {
        write_receipt_if_requested(&receipt)?;
        return Ok(receipt);
    }

    let mut command = std::process::Command::new(&target.command);
    command
        .args(args)
        .current_dir(&cwd)
        .env("SCRIPT_KIT_PROMPT", &payload.prompt)
        .env("SCRIPT_KIT_PROMPT_SHA256", &receipt.prompt_sha256)
        .env("SCRIPT_KIT_PROMPT_TARGET_ID", &target.id);
    for (key, value) in env {
        command.env(key, value);
    }
    let child = command
        .spawn()
        .map_err(|error| AgentPromptHandoffError::Spawn(error.to_string()))?;
    receipt.spawned = true;
    receipt.pid = Some(child.id());
    write_receipt_if_requested(&receipt)?;
    Ok(receipt)
}

fn export_prompt_to_file(
    payload: &AgentPromptHandoffPayload,
    action: AgentPromptActionId,
) -> Result<AgentPromptExportReceipt, AgentPromptHandoffError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ExternalStorage)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    let dry_run = env_truthy(DRY_RUN_ENV);
    let prompt_sha256 = sha256_hex(&payload.prompt);
    let export_dir = prompt_export_dir();
    let path = export_dir.join(prompt_export_filename(&prompt_sha256));

    let mut receipt = export_receipt_for_payload(
        payload,
        action,
        dry_run,
        prompt_sha256,
        "file",
        "prompt_export_file",
    );
    receipt.path = Some(path.to_string_lossy().to_string());

    if dry_run {
        write_export_receipt_if_requested(&receipt)?;
        return Ok(receipt);
    }

    ensure_private_handoff_directory(&export_dir)?;
    write_private_handoff_file(&path, payload.prompt.as_bytes())?;
    receipt.path = Some(path.to_string_lossy().to_string());
    write_export_receipt_if_requested(&receipt)?;
    Ok(receipt)
}

fn export_prompt_to_gist(
    payload: &AgentPromptHandoffPayload,
    action: AgentPromptActionId,
) -> Result<AgentPromptExportReceipt, AgentPromptHandoffError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)
        .map_err(|error| AgentPromptHandoffError::Spawn(error.to_string()))?;
    let dry_run = env_truthy(DRY_RUN_ENV);
    let prompt_sha256 = sha256_hex(&payload.prompt);
    let gh_binary = std::env::var(GH_BINARY_ENV).unwrap_or_else(|_| "gh".to_string());
    let filename = prompt_export_filename(&prompt_sha256);
    let mut receipt = export_receipt_for_payload(
        payload,
        action,
        dry_run,
        prompt_sha256,
        "gist",
        "prompt_export_gist_private",
    );

    if dry_run {
        write_export_receipt_if_requested(&receipt)?;
        return Ok(receipt);
    }

    let dir =
        create_private_handoff_directory(&std::env::temp_dir().join("script-kit-prompt-export"))?;
    let path = dir.join(&filename);
    write_private_handoff_file(&path, payload.prompt.as_bytes())?;
    receipt.path = Some(path.to_string_lossy().to_string());

    let output = std::process::Command::new(&gh_binary)
        .args(["gist", "create"])
        .arg(&path)
        .args(["--private", "--filename", &filename])
        .output()
        .map_err(|error| AgentPromptHandoffError::Spawn(error.to_string()))?;
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
    if !output.status.success() {
        return Err(AgentPromptHandoffError::Spawn(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    receipt.spawned = true;
    receipt.path = None;
    receipt.url = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
    write_export_receipt_if_requested(&receipt)?;
    Ok(receipt)
}

fn copy_prompt_to_clipboard(
    payload: &AgentPromptHandoffPayload,
    action: AgentPromptActionId,
) -> Result<AgentPromptExportReceipt, AgentPromptHandoffError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    copy_prompt_to_clipboard_with_writer(payload, action, |prompt| {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
        clipboard
            .set_text(prompt.to_string())
            .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))
    })
}

fn copy_prompt_to_clipboard_with_writer<F>(
    payload: &AgentPromptHandoffPayload,
    action: AgentPromptActionId,
    write_clipboard: F,
) -> Result<AgentPromptExportReceipt, AgentPromptHandoffError>
where
    F: FnOnce(&str) -> Result<(), AgentPromptHandoffError>,
{
    let dry_run = env_truthy(DRY_RUN_ENV);
    let prompt_sha256 = sha256_hex(&payload.prompt);
    let mut receipt = export_receipt_for_payload(
        payload,
        action,
        dry_run,
        prompt_sha256,
        "clipboard",
        "prompt_copy_clipboard",
    );

    if dry_run {
        write_export_receipt_if_requested(&receipt)?;
        return Ok(receipt);
    }

    write_clipboard(&payload.prompt)?;
    receipt.clipboard_written = true;
    write_export_receipt_if_requested(&receipt)?;
    Ok(receipt)
}

fn export_receipt_for_payload(
    payload: &AgentPromptHandoffPayload,
    action: AgentPromptActionId,
    dry_run: bool,
    prompt_sha256: String,
    export_kind: &str,
    command_kind: &str,
) -> AgentPromptExportReceipt {
    AgentPromptExportReceipt {
        action_id: action.action_id().to_string(),
        dry_run,
        cwd: payload.cwd.to_string_lossy().to_string(),
        prompt_chars: payload.prompt.chars().count(),
        prompt_sha256,
        context_part_count: payload.context_part_count,
        prompt_builder_segment_count: payload.prompt_builder_segment_count,
        export_kind: export_kind.to_string(),
        path: None,
        url: None,
        command_kind: command_kind.to_string(),
        clipboard_written: false,
        spawned: false,
    }
}

fn prompt_export_dir() -> PathBuf {
    std::env::var(PROMPT_EXPORT_DIR_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| PathBuf::from(shellexpand::tilde(&value).to_string()))
        .unwrap_or_else(|| crate::setup::get_kit_path().join("prompt-exports"))
}

fn prompt_export_filename(prompt_sha256: &str) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let short_hash = prompt_sha256.get(..12).unwrap_or(prompt_sha256);
    format!("prompt-{timestamp}-{short_hash}.md")
}

fn replace_prompt_placeholders(value: &str, prompt: &str, prompt_file: &str) -> String {
    value
        .replace("{prompt}", prompt)
        .replace("{promptFile}", prompt_file)
}

struct PreparedCmuxCodexWrapper {
    command_string: String,
}

fn prepare_cmux_codex_wrapper(
    prompt: &str,
    codex_binary: &str,
    cwd: &Path,
) -> Result<PreparedCmuxCodexWrapper, AgentPromptHandoffError> {
    prepare_cmux_codex_wrapper_at(
        prompt,
        codex_binary,
        cwd,
        &std::env::temp_dir().join("script-kit-agent-handoff"),
    )
}

fn prepare_cmux_codex_wrapper_at(
    prompt: &str,
    codex_binary: &str,
    cwd: &Path,
    handoff_root: &Path,
) -> Result<PreparedCmuxCodexWrapper, AgentPromptHandoffError> {
    let dir = create_private_handoff_directory(handoff_root)?;

    let prompt_path = dir.join("prompt.md");
    let script_path = dir.join("run.zsh");
    write_private_handoff_file(&prompt_path, prompt.as_bytes())?;

    let script = format!(
        "#!/bin/zsh\nset -euo pipefail\nscript_path=\"${{0:A}}\"\nhandoff_dir=\"${{script_path:h}}\"\nprompt_file=\"$handoff_dir/prompt.md\"\ncleanup() {{\n  rm -f \"$prompt_file\" \"$script_path\"\n  rmdir \"$handoff_dir\" 2>/dev/null || true\n}}\ntrap cleanup EXIT\npython3 - \"$prompt_file\" \"$script_path\" \"$handoff_dir\" {} {} <<'PY'\nimport os\nimport sys\n\nprompt_file, script_path, handoff_dir, codex_binary, cwd = sys.argv[1:]\nwith open(prompt_file, 'rb') as handle:\n    prompt = handle.read().decode('utf-8')\nfor path in (prompt_file, script_path):\n    try:\n        os.unlink(path)\n    except FileNotFoundError:\n        pass\ntry:\n    os.rmdir(handoff_dir)\nexcept OSError:\n    pass\nos.chdir(cwd)\nos.execvp(codex_binary, [codex_binary, '--cd', cwd, '--', prompt])\nPY\n",
        shell_quote(codex_binary),
        shell_quote_path(cwd)
    );
    write_private_handoff_file(&script_path, script.as_bytes())?;
    set_file_mode(&script_path, 0o700)?;

    Ok(PreparedCmuxCodexWrapper {
        command_string: format!("/bin/zsh {}", shell_quote_path(&script_path)),
    })
}

fn build_cmux_workspace_create_rpc_args(
    cwd: &Path,
) -> Result<Vec<String>, AgentPromptHandoffError> {
    let params = serde_json::json!({
        "title": "Script Kit Codex Handoff",
        "working_directory": cwd.to_string_lossy(),
        "focus": true,
        "eager_load_terminal": true,
    });
    let params_json = serde_json::to_string(&params)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    Ok(vec![
        "rpc".to_string(),
        "workspace.create".to_string(),
        params_json,
    ])
}

fn build_cmux_surface_create_rpc_args(
    workspace_ref: &str,
    cwd: &Path,
    command_string: &str,
) -> Result<Vec<String>, AgentPromptHandoffError> {
    let params = serde_json::json!({
        "workspace_id": workspace_ref,
        "type": "terminal",
        "working_directory": cwd.to_string_lossy(),
        "initial_command": command_string,
        "tmux_start_command": command_string,
        "focus": true,
    });
    let params_json = serde_json::to_string(&params)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    Ok(vec![
        "rpc".to_string(),
        "surface.create".to_string(),
        params_json,
    ])
}

fn parse_cmux_workspace_ref(stdout: &[u8]) -> Result<String, AgentPromptHandoffError> {
    let value: serde_json::Value = serde_json::from_slice(stdout).map_err(|error| {
        AgentPromptHandoffError::Spawn(format!(
            "cmux workspace.create returned invalid JSON: {error}"
        ))
    })?;
    value
        .get("workspace_ref")
        .or_else(|| value.get("workspace_id"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_string())
        .ok_or_else(|| {
            AgentPromptHandoffError::Spawn(
                "cmux workspace.create did not return workspace_ref".to_string(),
            )
        })
}

fn ensure_private_handoff_directory(path: &Path) -> Result<(), AgentPromptHandoffError> {
    crate::atomic_file::ensure_private_directory(path)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))
}

fn create_private_handoff_directory(root: &Path) -> Result<PathBuf, AgentPromptHandoffError> {
    ensure_private_handoff_directory(root)?;
    let directory = root.join(uuid::Uuid::new_v4().to_string());
    ensure_private_handoff_directory(&directory)?;
    Ok(directory)
}

fn write_private_handoff_file(path: &Path, bytes: &[u8]) -> Result<(), AgentPromptHandoffError> {
    crate::atomic_file::write_private_atomic(path, bytes)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))
}

fn set_file_mode(path: &Path, mode: u32) -> Result<(), AgentPromptHandoffError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?
            .permissions();
        permissions.set_mode(mode);
        std::fs::set_permissions(path, permissions)
            .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}

fn write_receipt_if_requested(
    receipt: &AgentPromptHandoffReceipt,
) -> Result<(), AgentPromptHandoffError> {
    let Ok(path) = std::env::var(RECEIPT_PATH_ENV) else {
        return Ok(());
    };
    let path = PathBuf::from(path);
    let json = serde_json::to_string_pretty(receipt)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    write_private_handoff_file(&path, json.as_bytes())
}

fn write_export_receipt_if_requested(
    receipt: &AgentPromptExportReceipt,
) -> Result<(), AgentPromptHandoffError> {
    let Ok(path) = std::env::var(RECEIPT_PATH_ENV) else {
        return Ok(());
    };
    let path = PathBuf::from(path);
    let json = serde_json::to_string_pretty(receipt)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    write_private_handoff_file(&path, json.as_bytes())
}

fn env_truthy(name: &str) -> bool {
    std::env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn shell_quote_path(path: &Path) -> String {
    shell_quote(&path.to_string_lossy())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
include!("agent_prompt_handoff_tests.rs");

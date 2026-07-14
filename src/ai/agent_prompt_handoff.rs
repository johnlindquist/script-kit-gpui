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
    let prepared = crate::ai::message_parts::prepare_user_message_with_receipt(
        &normalized_prompt,
        &context_parts,
        &scripts,
        &scriptlets,
    );
    if prepared.decision == PreparedMessageDecision::Blocked {
        return Err(AgentPromptHandoffError::UnsupportedPrompt(
            prepared
                .user_error
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
        let dir = std::env::temp_dir()
            .join("script-kit-agent-handoff")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir)
            .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
        set_file_mode(&dir, 0o700)?;
        let path = dir.join("prompt.md");
        std::fs::write(&path, &payload.prompt)
            .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
        set_file_mode(&path, 0o600)?;
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

    std::fs::create_dir_all(&export_dir)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    set_file_mode(&export_dir, 0o700)?;
    std::fs::write(&path, &payload.prompt)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    set_file_mode(&path, 0o600)?;
    receipt.path = Some(path.to_string_lossy().to_string());
    write_export_receipt_if_requested(&receipt)?;
    Ok(receipt)
}

fn export_prompt_to_gist(
    payload: &AgentPromptHandoffPayload,
    action: AgentPromptActionId,
) -> Result<AgentPromptExportReceipt, AgentPromptHandoffError> {
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

    let dir = std::env::temp_dir()
        .join("script-kit-prompt-export")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    set_file_mode(&dir, 0o700)?;
    let path = dir.join(&filename);
    std::fs::write(&path, &payload.prompt)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    set_file_mode(&path, 0o600)?;
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
    let dir = std::env::temp_dir()
        .join("script-kit-agent-handoff")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    set_file_mode(&dir, 0o700)?;

    let prompt_path = dir.join("prompt.md");
    let script_path = dir.join("run.zsh");
    std::fs::write(&prompt_path, prompt)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    set_file_mode(&prompt_path, 0o600)?;

    let script = format!(
        "#!/bin/zsh\nset -euo pipefail\nscript_path=\"${{0:A}}\"\nhandoff_dir=\"${{script_path:h}}\"\nprompt_file=\"$handoff_dir/prompt.md\"\ncleanup() {{\n  rm -f \"$prompt_file\" \"$script_path\"\n  rmdir \"$handoff_dir\" 2>/dev/null || true\n}}\ntrap cleanup EXIT\npython3 - \"$prompt_file\" \"$script_path\" \"$handoff_dir\" {} {} <<'PY'\nimport os\nimport sys\n\nprompt_file, script_path, handoff_dir, codex_binary, cwd = sys.argv[1:]\nwith open(prompt_file, 'rb') as handle:\n    prompt = handle.read().decode('utf-8')\nfor path in (prompt_file, script_path):\n    try:\n        os.unlink(path)\n    except FileNotFoundError:\n        pass\ntry:\n    os.rmdir(handoff_dir)\nexcept OSError:\n    pass\nos.chdir(cwd)\nos.execvp(codex_binary, [codex_binary, '--cd', cwd, '--', prompt])\nPY\n",
        shell_quote(codex_binary),
        shell_quote_path(cwd)
    );
    std::fs::write(&script_path, script)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    }
    let json = serde_json::to_string_pretty(receipt)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    std::fs::write(path, json).map_err(|error| AgentPromptHandoffError::Io(error.to_string()))
}

fn write_export_receipt_if_requested(
    receipt: &AgentPromptExportReceipt,
) -> Result<(), AgentPromptHandoffError> {
    let Ok(path) = std::env::var(RECEIPT_PATH_ENV) else {
        return Ok(());
    };
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    }
    let json = serde_json::to_string_pretty(receipt)
        .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))?;
    std::fs::write(path, json).map_err(|error| AgentPromptHandoffError::Io(error.to_string()))
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
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    fn env_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_env_test() -> std::sync::MutexGuard<'static, ()> {
        env_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn adapter_from_action_id_recognizes_cmux_codex() {
        assert_eq!(
            adapter_from_action_id(CMUX_CODEX_ACTION_ID),
            Some(AgentPromptHandoffAdapterId::CmuxCodex)
        );
        assert_eq!(adapter_from_action_id("agent_chat:handoff:other"), None);
    }

    #[test]
    fn cmux_args_do_not_include_raw_prompt() {
        let prompt = "summarize this private prompt";
        let workspace_args =
            build_cmux_workspace_create_rpc_args(Path::new("/Users/example/project"))
                .expect("build workspace args");
        let surface_args = build_cmux_surface_create_rpc_args(
            "workspace:99",
            Path::new("/Users/example/project"),
            "/bin/zsh '/tmp/script-kit-agent-handoff/abc/run.zsh'",
        )
        .expect("build surface args");
        let joined = workspace_args
            .iter()
            .chain(surface_args.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!joined.contains(prompt));
        assert_eq!(workspace_args[0], "rpc");
        assert_eq!(workspace_args[1], "workspace.create");
        let workspace_params: serde_json::Value =
            serde_json::from_str(&workspace_args[2]).expect("workspace rpc params");
        assert_eq!(
            workspace_params["working_directory"],
            "/Users/example/project"
        );
        assert_eq!(workspace_params["focus"], true);
        assert_eq!(workspace_params["eager_load_terminal"], true);
        assert!(workspace_params.get("initial_command").is_none());

        assert_eq!(surface_args[0], "rpc");
        assert_eq!(surface_args[1], "surface.create");
        let surface_params: serde_json::Value =
            serde_json::from_str(&surface_args[2]).expect("surface rpc params");
        assert_eq!(surface_params["workspace_id"], "workspace:99");
        assert_eq!(
            surface_params["working_directory"],
            "/Users/example/project"
        );
        assert_eq!(
            surface_params["initial_command"],
            "/bin/zsh '/tmp/script-kit-agent-handoff/abc/run.zsh'"
        );
        assert_eq!(
            surface_params["tmux_start_command"],
            "/bin/zsh '/tmp/script-kit-agent-handoff/abc/run.zsh'"
        );
        assert_eq!(surface_params["focus"], true);
    }

    #[test]
    fn receipt_does_not_serialize_raw_prompt() {
        let receipt = AgentPromptHandoffReceipt {
            adapter_id: CMUX_CODEX_ADAPTER_ID.to_string(),
            action_id: CMUX_CODEX_ACTION_ID.to_string(),
            dry_run: true,
            cwd: "/tmp".to_string(),
            prompt_chars: 26,
            prompt_sha256: sha256_hex("summarize private prompt"),
            command_kind: "cmux_workspace_surface_create_initial_command".to_string(),
            cmux_binary: "cmux".to_string(),
            codex_binary: "codex".to_string(),
            prompt_file_created: false,
            script_file_created: false,
            spawned: false,
            pid: None,
        };
        let json = serde_json::to_string(&receipt).expect("serialize receipt");
        assert!(!json.contains("summarize private prompt"));
        assert!(json.contains("\"promptSha256\""));
        assert!(json.contains("\"promptChars\""));
    }

    #[test]
    fn export_receipt_does_not_serialize_raw_prompt() {
        let receipt = AgentPromptExportReceipt {
            action_id: EXPORT_FILE_ACTION_ID.to_string(),
            dry_run: true,
            cwd: "/tmp".to_string(),
            prompt_chars: 26,
            prompt_sha256: sha256_hex("summarize private prompt"),
            context_part_count: 2,
            prompt_builder_segment_count: 3,
            export_kind: "file".to_string(),
            path: Some("/tmp/prompt.md".to_string()),
            url: None,
            command_kind: "prompt_export_file".to_string(),
            clipboard_written: false,
            spawned: false,
        };
        let json = serde_json::to_string(&receipt).expect("serialize receipt");
        assert!(!json.contains("summarize private prompt"));
        assert!(json.contains("\"promptSha256\""));
        assert!(json.contains("\"promptChars\""));
        assert!(json.contains("\"contextPartCount\""));
        assert!(json.contains("\"promptBuilderSegmentCount\""));
        assert!(json.contains("\"clipboardWritten\""));
    }

    #[test]
    fn export_file_writes_prompt_to_configured_directory() {
        let _guard = lock_env_test();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let receipt_path = temp_dir.path().join("receipt.json");
        let export_dir = temp_dir.path().join("exports");
        let prompt = "file export proof prompt";
        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (GH_BINARY_ENV, None),
            (
                RECEIPT_PATH_ENV,
                Some(receipt_path.to_string_lossy().to_string()),
            ),
            (
                PROMPT_EXPORT_DIR_ENV,
                Some(export_dir.to_string_lossy().to_string()),
            ),
        ]);

        let receipt = export_prompt(&test_payload(prompt), AgentPromptActionId::ExportFile)
            .expect("export prompt");

        assert!(!receipt.dry_run);
        assert_eq!(receipt.action_id, EXPORT_FILE_ACTION_ID);
        assert_eq!(receipt.export_kind, "file");
        assert_eq!(receipt.command_kind, "prompt_export_file");
        assert_eq!(receipt.prompt_sha256, sha256_hex(prompt));
        assert_eq!(receipt.context_part_count, 0);
        assert_eq!(receipt.prompt_builder_segment_count, 0);
        assert!(!receipt.clipboard_written);
        let path = PathBuf::from(receipt.path.as_deref().expect("export path"));
        assert!(path.starts_with(&export_dir));
        assert_eq!(
            std::fs::read_to_string(&path).expect("exported prompt"),
            prompt
        );
        let serialized_receipt =
            std::fs::read_to_string(&receipt_path).expect("serialized export receipt");
        assert!(serialized_receipt.contains("\"prompt_export_file\""));
        assert!(!serialized_receipt.contains(prompt));
    }

    #[test]
    fn export_gist_uses_private_gh_gist_create_without_leaking_prompt_in_receipt() {
        let _guard = lock_env_test();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let gh_stub_path = temp_dir.path().join("gh-stub.py");
        let gh_receipt_path = temp_dir.path().join("gh-receipt.json");
        let export_receipt_path = temp_dir.path().join("export-receipt.json");
        let prompt = "gist export proof prompt";
        std::fs::write(
            &gh_stub_path,
            r#"#!/usr/bin/env python3
import hashlib
import json
import os
import sys

args = sys.argv[1:]
prompt_path = args[2]
with open(prompt_path, 'r') as handle:
    prompt = handle.read()
with open(os.environ['GH_STUB_RECEIPT'], 'w') as handle:
    json.dump({
        'argv': args,
        'promptSha256': hashlib.sha256(prompt.encode()).hexdigest(),
        'hasPrivateFlag': '--private' in args,
        'hasFilenameFlag': '--filename' in args,
    }, handle, indent=2)
print('https://gist.github.com/fake/private-gist')
"#,
        )
        .expect("write gh stub");
        set_file_mode(&gh_stub_path, 0o700).expect("chmod gh stub");
        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (
                RECEIPT_PATH_ENV,
                Some(export_receipt_path.to_string_lossy().to_string()),
            ),
            (
                GH_BINARY_ENV,
                Some(gh_stub_path.to_string_lossy().to_string()),
            ),
            (
                "GH_STUB_RECEIPT",
                Some(gh_receipt_path.to_string_lossy().to_string()),
            ),
            (PROMPT_EXPORT_DIR_ENV, None),
        ]);

        let receipt = export_prompt(&test_payload(prompt), AgentPromptActionId::ExportGist)
            .expect("export gist");

        assert_eq!(receipt.action_id, EXPORT_GIST_ACTION_ID);
        assert_eq!(receipt.export_kind, "gist");
        assert_eq!(receipt.command_kind, "prompt_export_gist_private");
        assert_eq!(receipt.context_part_count, 0);
        assert_eq!(receipt.prompt_builder_segment_count, 0);
        assert!(!receipt.clipboard_written);
        assert_eq!(
            receipt.url.as_deref(),
            Some("https://gist.github.com/fake/private-gist")
        );
        assert!(receipt.spawned);
        assert_eq!(receipt.path, None);

        let gh_receipt = std::fs::read_to_string(&gh_receipt_path).expect("gh receipt");
        assert!(gh_receipt.contains("\"gist\""));
        assert!(gh_receipt.contains("\"create\""));
        assert!(gh_receipt.contains("\"hasPrivateFlag\": true"));
        assert!(gh_receipt.contains("\"hasFilenameFlag\": true"));
        assert!(gh_receipt.contains(&sha256_hex(prompt)));
        let export_receipt = std::fs::read_to_string(&export_receipt_path).expect("export receipt");
        assert!(export_receipt.contains("\"prompt_export_gist_private\""));
        assert!(!export_receipt.contains(prompt));
    }

    #[test]
    fn builtin_prompt_actions_include_copy_prompt_clipboard_action() {
        let actions = builtin_prompt_actions();
        assert!(actions.contains(&AgentPromptActionId::CopyPrompt));
        assert_eq!(
            prompt_action_from_action_id(COPY_PROMPT_ACTION_ID),
            Some(AgentPromptActionId::CopyPrompt)
        );
        assert_eq!(
            AgentPromptActionId::CopyPrompt.id(),
            COPY_PROMPT_PROMPT_ACTION_ID
        );
        assert_eq!(
            AgentPromptActionId::CopyPrompt.action_id(),
            COPY_PROMPT_ACTION_ID
        );
        assert_eq!(
            prompt_action_id(COPY_PROMPT_PROMPT_ACTION_ID),
            COPY_PROMPT_ACTION_ID
        );
    }

    #[test]
    fn copy_prompt_dry_run_receipt_hash_matches_exact_prompt_without_clipboard_write() {
        let _guard = lock_env_test();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let receipt_path = temp_dir.path().join("copy-receipt.json");
        let prompt = "copy prompt proof";
        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, Some("1".to_string())),
            (
                RECEIPT_PATH_ENV,
                Some(receipt_path.to_string_lossy().to_string()),
            ),
        ]);

        let receipt = copy_prompt_to_clipboard_with_writer(
            &test_payload(prompt),
            AgentPromptActionId::CopyPrompt,
            |_| panic!("dry-run copy must not write clipboard"),
        )
        .expect("copy prompt dry-run");

        assert!(receipt.dry_run);
        assert_eq!(receipt.action_id, COPY_PROMPT_ACTION_ID);
        assert_eq!(receipt.export_kind, "clipboard");
        assert_eq!(receipt.command_kind, "prompt_copy_clipboard");
        assert_eq!(receipt.prompt_sha256, sha256_hex(prompt));
        assert!(!receipt.clipboard_written);
        let serialized_receipt =
            std::fs::read_to_string(&receipt_path).expect("serialized copy receipt");
        assert!(serialized_receipt.contains("\"prompt_copy_clipboard\""));
        assert!(!serialized_receipt.contains(prompt));
    }

    #[test]
    fn copy_prompt_to_clipboard_writer_receives_exact_prompt() {
        let _guard = lock_env_test();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let copied_path = temp_dir.path().join("copied.txt");
        let receipt_path = temp_dir.path().join("copy-receipt.json");
        let prompt = "copy prompt exact payload\nwith newline";
        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (
                RECEIPT_PATH_ENV,
                Some(receipt_path.to_string_lossy().to_string()),
            ),
        ]);

        let receipt = copy_prompt_to_clipboard_with_writer(
            &test_payload(prompt),
            AgentPromptActionId::CopyPrompt,
            |value| {
                std::fs::write(&copied_path, value)
                    .map_err(|error| AgentPromptHandoffError::Io(error.to_string()))
            },
        )
        .expect("copy prompt");

        assert!(!receipt.dry_run);
        assert_eq!(receipt.export_kind, "clipboard");
        assert_eq!(receipt.command_kind, "prompt_copy_clipboard");
        assert!(receipt.clipboard_written);
        assert_eq!(
            std::fs::read_to_string(&copied_path).expect("copied prompt"),
            prompt
        );
        let serialized_receipt =
            std::fs::read_to_string(&receipt_path).expect("serialized copy receipt");
        assert!(serialized_receipt.contains("\"clipboardWritten\": true"));
        assert!(!serialized_receipt.contains(prompt));
    }

    #[test]
    fn handoff_compiler_preserves_every_ai_context_part_variant() {
        let fixture = RichPromptContextFixture::new();
        assert_ai_context_part_variant_coverage(&fixture.parts);

        let payload = compile_fixture_payload(&fixture);

        assert_eq!(payload.context_part_count, fixture.parts.len());
        assert_eq!(payload.prompt_builder_segment_count, 1);
        assert_compiled_prompt_contains_all_context_fingerprints(&payload.prompt, &fixture);
        assert!(
            !payload
                .prompt
                .contains("PROMPT_EXPORT_AMBIENT_DISPLAY_ONLY_SENTINEL"),
            "AmbientContext is display-only; staged content must arrive as a ResourceUri or TextBlock"
        );
    }

    #[test]
    fn ambient_context_export_policy_is_explicit() {
        let part = AiContextPart::AmbientContext {
            label: "PROMPT_EXPORT_AMBIENT_DISPLAY_ONLY_SENTINEL".to_string(),
        };
        let prepared =
            crate::ai::message_parts::prepare_user_message_with_receipt("ask", &[part], &[], &[]);

        assert_eq!(prepared.context.attempted, 1);
        assert_eq!(prepared.context.resolved, 0);
        assert_eq!(prepared.final_user_content, "ask");
        assert_eq!(
            prepared.outcomes[0].kind,
            crate::ai::message_parts::ContextPartPreparationOutcomeKind::DisplayOnly
        );
    }

    #[test]
    fn prompt_actions_share_identical_compiled_prompt_hash_for_file_and_clipboard() {
        let _guard = lock_env_test();
        let fixture = RichPromptContextFixture::new();
        let payload = compile_fixture_payload(&fixture);
        let export_dir = fixture.temp_dir.path().join("exports");
        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (RECEIPT_PATH_ENV, None),
            (
                PROMPT_EXPORT_DIR_ENV,
                Some(export_dir.to_string_lossy().to_string()),
            ),
        ]);

        let file_receipt =
            export_prompt(&payload, AgentPromptActionId::ExportFile).expect("file export");
        let copied = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let copied_for_writer = copied.clone();
        let copy_receipt = copy_prompt_to_clipboard_with_writer(
            &payload,
            AgentPromptActionId::CopyPrompt,
            move |value| {
                *copied_for_writer.lock().expect("copy lock") = value.to_string();
                Ok(())
            },
        )
        .expect("copy prompt");

        assert_export_receipt_matches_payload(
            &file_receipt,
            &payload,
            "file",
            "prompt_export_file",
        );
        assert_export_receipt_matches_payload(
            &copy_receipt,
            &payload,
            "clipboard",
            "prompt_copy_clipboard",
        );
        assert_eq!(file_receipt.prompt_sha256, copy_receipt.prompt_sha256);
        assert_eq!(file_receipt.prompt_chars, copy_receipt.prompt_chars);
        assert_eq!(
            file_receipt.context_part_count,
            copy_receipt.context_part_count
        );
        assert_eq!(
            file_receipt.prompt_builder_segment_count,
            copy_receipt.prompt_builder_segment_count
        );
        let exported_path = PathBuf::from(file_receipt.path.as_deref().expect("export path"));
        assert_eq!(
            std::fs::read_to_string(exported_path).expect("exported rich prompt"),
            payload.prompt
        );
        assert_eq!(*copied.lock().expect("copied prompt"), payload.prompt);
        assert!(!file_receipt.clipboard_written);
        assert!(copy_receipt.clipboard_written);
    }

    #[test]
    fn handoff_compiler_matches_spine_submit_plan_for_prompt_builder_inputs() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let file_path = temp_dir.path().join("brief.txt");
        std::fs::write(&file_path, "briefing contents").expect("write fixture file");
        let file_input = format!("@file:{} summarize", file_path.to_string_lossy());
        let profile_file_input = format!(
            "|creative /rewrite @file:{} make it useful",
            file_path.to_string_lossy()
        );
        let cases = [
            (
                "/rewrite make this concise",
                None,
                Some("/rewrite\n\nmake this concise"),
            ),
            (
                profile_file_input.as_str(),
                Some("creative"),
                Some("briefing contents"),
            ),
            // Styles are not profiles: `.professional` carries an explicit
            // tone instruction in the prompt and selects no profile.
            (
                ".professional make it shorter",
                None,
                Some("professional workplace tone"),
            ),
            (">:demo explain setup", None, Some("explain setup")),
            ("@unknownThing summarize", None, Some("Preflight warning")),
            (file_input.as_str(), None, Some("briefing contents")),
        ];

        for (raw, expected_profile, expected_prompt_fragment) in cases {
            let parse = crate::spine::parse_spine(raw);
            let plan = crate::spine::prompt_plan::build_spine_prompt_plan(&parse);
            assert!(plan.should_submit_to_chat(), "{raw} should be submittable");

            let scripts: Vec<std::sync::Arc<crate::scripts::Script>> = Vec::new();
            let scriptlets: Vec<std::sync::Arc<crate::scripts::Scriptlet>> = Vec::new();
            let expected = crate::ai::message_parts::prepare_user_message_with_receipt(
                plan.normalized_prompt.trim(),
                &plan.context_parts,
                &scripts,
                &scriptlets,
            );
            let result = compile_handoff_payload_from_spine_plan(
                AgentPromptHandoffAdapterId::CmuxCodex,
                raw.to_string(),
                PathBuf::from("/tmp/project"),
                Some("gpt-test".to_string()),
                Vec::new(),
                plan.clone(),
            );

            if expected.decision == PreparedMessageDecision::Blocked {
                assert!(
                    matches!(result, Err(AgentPromptHandoffError::UnsupportedPrompt(_))),
                    "{raw} should block like normal message preparation"
                );
                continue;
            }

            let payload =
                result.unwrap_or_else(|error| panic!("compile {raw}: {}", error.user_message()));

            let expected_content = expected.final_user_content.trim();
            if plan.context_parts.is_empty() {
                assert_eq!(payload.prompt, expected_content, "{raw}");
            } else {
                let normalized_prompt = plan.normalized_prompt.trim();
                assert!(
                    normalized_prompt.is_empty() || payload.prompt.contains(normalized_prompt),
                    "{raw} payload prompt did not contain normalized prompt {normalized_prompt:?}: {:?}",
                    payload.prompt
                );
            }
            assert_eq!(
                payload.prompt_builder_segment_count, plan.prompt_builder_segment_count,
                "{raw}"
            );
            assert_eq!(
                payload.context_part_count,
                plan.context_parts.len(),
                "{raw}"
            );
            assert_eq!(payload.profile_id.as_deref(), expected_profile, "{raw}");
            if let Some(fragment) = expected_prompt_fragment {
                assert!(
                    payload.prompt.contains(fragment),
                    "{raw} prompt did not contain {fragment:?}: {:?}",
                    payload.prompt
                );
            }
        }
    }

    #[test]
    fn handoff_compiler_blocks_non_submittable_prompt_builder_and_mode_inputs() {
        for raw in [
            "@clip",
            ">",
            ";todo Buy milk",
            ":type:script git",
            "!echo hi",
            "?help",
            "~note",
        ] {
            let parse = crate::spine::parse_spine(raw);
            let plan = crate::spine::prompt_plan::build_spine_prompt_plan(&parse);
            let result = compile_handoff_payload_from_spine_plan(
                AgentPromptHandoffAdapterId::CmuxCodex,
                raw.to_string(),
                PathBuf::from("/tmp/project"),
                None,
                Vec::new(),
                plan,
            );

            assert!(
                matches!(result, Err(AgentPromptHandoffError::UnsupportedPrompt(_))),
                "{raw} should be blocked, got {result:?}"
            );
        }
    }

    #[test]
    fn handoff_policy_covers_all_main_input_spine_construct_classes() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Expected {
            Submit,
            PlainFallback,
            Block,
        }

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let file_path = temp_dir.path().join("brief.txt");
        std::fs::write(&file_path, "briefing contents").expect("write fixture file");
        let file_input = format!("@file:{} summarize this", file_path.to_string_lossy());
        let cases = [
            (
                "free text fallback",
                "plain request",
                Expected::PlainFallback,
            ),
            (
                "context mention builtin",
                "@diagnostics explain this",
                Expected::Submit,
            ),
            (
                "context mention file",
                file_input.as_str(),
                Expected::Submit,
            ),
            (
                "context mention unknown",
                "@unknownThing explain",
                Expected::Submit,
            ),
            (
                "slash command",
                "/rewrite make this concise",
                Expected::Submit,
            ),
            ("profile", "|creative brainstorm options", Expected::Submit),
            (
                "style sugar",
                ".professional make it shorter",
                Expected::Submit,
            ),
            (
                "project cwd",
                ">:demo inspect this project",
                Expected::Submit,
            ),
            ("capture syntax", ";todo Buy milk", Expected::Block),
            ("list filter", ":type:script git", Expected::Block),
            ("mode exit shell", "!echo hi", Expected::Block),
            ("mode exit help", "?help", Expected::Block),
            ("mode exit note", "~note", Expected::Block),
            ("incomplete context draft", "@clip", Expected::Block),
            ("incomplete cwd draft", ">", Expected::Block),
        ];

        for (label, raw, expected) in cases {
            let parse = crate::spine::parse_spine(raw);
            let plan = crate::spine::prompt_plan::build_spine_prompt_plan(&parse);
            let result = compile_handoff_payload_from_spine_plan(
                AgentPromptHandoffAdapterId::CmuxCodex,
                raw.to_string(),
                PathBuf::from("/tmp/project"),
                None,
                Vec::new(),
                plan.clone(),
            );

            match expected {
                Expected::Submit => {
                    let payload = result.unwrap_or_else(|error| {
                        panic!("{label} should submit, got {}", error.user_message())
                    });
                    assert!(
                        payload.prompt_builder_segment_count > 0,
                        "{label} should use prompt-builder semantics"
                    );
                    assert!(
                        plan.should_submit_to_chat(),
                        "{label} should match Spine submit"
                    );
                }
                Expected::PlainFallback => {
                    let payload = result.unwrap_or_else(|error| {
                        panic!(
                            "{label} should hand off as plain text, got {}",
                            error.user_message()
                        )
                    });
                    assert_eq!(payload.prompt_builder_segment_count, 0, "{label}");
                    assert_eq!(payload.prompt, raw.trim(), "{label}");
                }
                Expected::Block => {
                    assert!(
                        matches!(result, Err(AgentPromptHandoffError::UnsupportedPrompt(_))),
                        "{label} should block, got {result:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn configured_prompt_target_launch_sets_prompt_env_and_placeholders() {
        let _guard = lock_env_test();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let target_path = temp_dir.path().join("target.py");
        let receipt_path = temp_dir.path().join("target-receipt.json");
        let handoff_receipt_path = temp_dir.path().join("handoff-receipt.json");
        let project_dir = temp_dir.path().join("project");
        std::fs::create_dir(&project_dir).expect("project dir");
        std::fs::write(
            &target_path,
            r#"#!/usr/bin/env python3
import hashlib
import json
import os
import pathlib
import sys

prompt = os.environ["SCRIPT_KIT_PROMPT"]
prompt_file = pathlib.Path(sys.argv[2])
with open(os.environ["TARGET_RECEIPT"], "w") as handle:
    json.dump({
        "argv": sys.argv[1:],
        "cwd": os.getcwd(),
        "promptSha256": hashlib.sha256(prompt.encode()).hexdigest(),
        "promptFileSha256": hashlib.sha256(prompt_file.read_text().encode()).hexdigest(),
        "targetId": os.environ["SCRIPT_KIT_PROMPT_TARGET_ID"],
        "customEnv": os.environ["CUSTOM_PROMPT"],
    }, handle, indent=2)
"#,
        )
        .expect("write target stub");
        set_file_mode(&target_path, 0o700).expect("chmod target stub");

        let prompt = "custom target prompt";
        let target = AgentPromptCommandTarget {
            id: "custom-app".to_string(),
            title: "Custom App".to_string(),
            description: None,
            command: target_path.to_string_lossy().to_string(),
            args: vec!["--prompt-file".to_string(), "{promptFile}".to_string()],
            cwd: Some(project_dir.clone()),
            env: HashMap::from([
                (
                    "TARGET_RECEIPT".to_string(),
                    receipt_path.to_string_lossy().to_string(),
                ),
                ("CUSTOM_PROMPT".to_string(), "{prompt}".to_string()),
            ]),
        };
        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (
                RECEIPT_PATH_ENV,
                Some(handoff_receipt_path.to_string_lossy().to_string()),
            ),
        ]);
        let payload = AgentPromptHandoffPayload {
            source: AgentPromptHandoffSource::AgentChatComposer,
            adapter_id: AgentPromptHandoffAdapterId::Command(target),
            raw_input: prompt.to_string(),
            prompt: prompt.to_string(),
            cwd: PathBuf::from("/tmp"),
            model_id: None,
            profile_id: None,
            context_part_count: 0,
            prompt_builder_segment_count: 0,
            warnings: Vec::new(),
        };

        let receipt = launch_prompt_handoff(&payload).expect("launch custom target");
        assert!(receipt.spawned);
        assert_eq!(receipt.action_id, "prompt-target/custom-app");
        wait_for_file(&receipt_path, Duration::from_secs(5)).expect("target receipt");

        let target_receipt = std::fs::read_to_string(&receipt_path).expect("target receipt");
        let target_receipt_json: serde_json::Value =
            serde_json::from_str(&target_receipt).expect("target receipt json");
        let actual_cwd = target_receipt_json["cwd"]
            .as_str()
            .map(PathBuf::from)
            .expect("target receipt cwd");
        let canonical_project_dir =
            std::fs::canonicalize(&project_dir).expect("canonical project dir");
        assert_eq!(
            std::fs::canonicalize(actual_cwd).expect("canonical target cwd"),
            canonical_project_dir
        );
        assert!(target_receipt.contains(&format!("\"promptSha256\": \"{}\"", sha256_hex(prompt))));
        assert!(
            target_receipt.contains(&format!("\"promptFileSha256\": \"{}\"", sha256_hex(prompt)))
        );
        assert!(target_receipt.contains("\"targetId\": \"custom-app\""));
        assert!(target_receipt.contains("\"customEnv\": \"custom target prompt\""));

        let handoff_receipt =
            std::fs::read_to_string(&handoff_receipt_path).expect("handoff receipt");
        assert!(handoff_receipt.contains("\"commandKind\": \"prompt_target_command\""));
        assert!(!handoff_receipt.contains(prompt));
    }

    #[test]
    fn handoff_compiler_preserves_plain_text_and_dedupes_attached_context() {
        let attached = AiContextPart::TextBlock {
            label: "Note".to_string(),
            source: "test://note".to_string(),
            text: "attached note".to_string(),
            mime_type: None,
        };
        let parse = crate::spine::parse_spine("plain question");
        let plan = crate::spine::prompt_plan::build_spine_prompt_plan(&parse);

        let payload = compile_handoff_payload_from_spine_plan(
            AgentPromptHandoffAdapterId::CmuxCodex,
            "plain question".to_string(),
            PathBuf::from("/tmp/project"),
            None,
            vec![attached.clone(), attached],
            plan,
        )
        .expect("plain text handoff with attached context");

        assert_eq!(payload.prompt_builder_segment_count, 0);
        assert_eq!(payload.context_part_count, 1);
        assert!(payload.prompt.contains("attached note"));
        assert!(payload.prompt.ends_with("plain question"));
    }

    #[test]
    fn handoff_compiler_blocks_when_context_preparation_blocks_normal_submit() {
        let missing_file = AiContextPart::FilePath {
            path: "/definitely/missing/script-kit-handoff.txt".to_string(),
            label: "missing.txt".to_string(),
        };
        let parse = crate::spine::parse_spine("plain question");
        let plan = crate::spine::prompt_plan::build_spine_prompt_plan(&parse);

        let result = compile_handoff_payload_from_spine_plan(
            AgentPromptHandoffAdapterId::CmuxCodex,
            "plain question".to_string(),
            PathBuf::from("/tmp/project"),
            None,
            vec![missing_file],
            plan,
        );

        assert!(
            matches!(result, Err(AgentPromptHandoffError::UnsupportedPrompt(ref reason)) if reason.contains("Failed to resolve context")),
            "missing context should block handoff like normal submit: {result:?}"
        );
    }

    #[test]
    fn cmux_codex_wrapper_uses_codex_cd_and_secure_temp_files() {
        let stub_dir = tempfile::tempdir().expect("stub dir");
        let stub_path = stub_dir.path().join("codex-stub.py");
        let receipt_path = stub_dir.path().join("codex-receipt.txt");
        let project_dir = stub_dir.path().join("project");
        std::fs::create_dir(&project_dir).expect("project dir");
        std::fs::write(
            &stub_path,
            "#!/usr/bin/env python3\nimport hashlib\nimport os\nimport pathlib\nimport sys\nprompt = sys.argv[4]\npathlib.Path(os.environ['CODEX_STUB_RECEIPT']).write_text('\\n'.join([\n    'pwd=' + os.getcwd(),\n    'argv0=' + sys.argv[1],\n    'argv1=' + sys.argv[2],\n    'argv2=' + sys.argv[3],\n    'prompt_sha=' + hashlib.sha256(prompt.encode()).hexdigest(),\n    'prompt_repr=' + repr(prompt),\n]))\n",
        )
        .expect("write codex stub");
        set_file_mode(&stub_path, 0o700).expect("chmod codex stub");
        let prompt = "prompt with trailing newline\n";
        let prepared =
            prepare_cmux_codex_wrapper(prompt, &stub_path.to_string_lossy(), &project_dir)
                .expect("prepare wrapper");
        let script_path = prepared
            .command_string
            .strip_prefix("/bin/zsh '")
            .and_then(|value| value.strip_suffix('\''))
            .map(PathBuf::from)
            .expect("quoted script path");
        let prompt_path = script_path.parent().expect("script dir").join("prompt.md");
        let script = std::fs::read_to_string(&script_path).expect("read wrapper");

        assert!(script.contains("os.execvp(codex_binary"));
        assert!(script.contains("'--cd', cwd, '--', prompt"));
        assert!(script.contains("os.unlink(path)"));
        assert!(!script.contains("$(cat"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir_mode = std::fs::metadata(script_path.parent().unwrap())
                .expect("dir metadata")
                .permissions()
                .mode()
                & 0o777;
            let prompt_mode = std::fs::metadata(&prompt_path)
                .expect("prompt metadata")
                .permissions()
                .mode()
                & 0o777;
            let script_mode = std::fs::metadata(&script_path)
                .expect("script metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir_mode, 0o700);
            assert_eq!(prompt_mode, 0o600);
            assert_eq!(script_mode, 0o700);
        }

        let status = std::process::Command::new("/bin/zsh")
            .arg(&script_path)
            .env("CODEX_STUB_RECEIPT", &receipt_path)
            .status()
            .expect("run wrapper");
        assert!(status.success(), "wrapper should launch codex stub");
        let receipt = std::fs::read_to_string(&receipt_path).expect("stub receipt");
        let canonical_project_dir =
            std::fs::canonicalize(&project_dir).expect("canonical project dir");
        assert!(receipt.contains(&format!("pwd={}", canonical_project_dir.to_string_lossy())));
        assert!(receipt.contains("argv0=--cd"));
        assert!(receipt.contains(&format!("argv1={}", project_dir.to_string_lossy())));
        assert!(receipt.contains("argv2=--"));
        assert!(receipt.contains(&format!("prompt_sha={}", sha256_hex(prompt))));
        assert!(receipt.contains("prompt_repr='prompt with trailing newline\\n'"));
        assert!(
            !prompt_path.exists(),
            "prompt file should be removed before Codex runs"
        );
        assert!(
            !script_path.exists(),
            "wrapper script should be removed before Codex runs"
        );
        assert!(
            !script_path.parent().expect("script dir").exists(),
            "handoff temp dir should be removed before Codex runs"
        );
    }

    #[test]
    fn launch_cmux_codex_spawns_cmux_stub_that_executes_codex_stub() {
        let _guard = lock_env_test();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let project_dir = temp_dir.path().join("project");
        std::fs::create_dir(&project_dir).expect("project dir");
        let cmux_stub_path = temp_dir.path().join("cmux-stub.py");
        let codex_stub_path = temp_dir.path().join("codex-stub.py");
        let cmux_receipt_path = temp_dir.path().join("cmux-receipt.json");
        let codex_receipt_path = temp_dir.path().join("codex-receipt.json");
        let handoff_receipt_path = temp_dir.path().join("handoff-receipt.json");
        let prompt = "cmux stub chain prompt\nwith trailing newline\n";

        std::fs::write(
            &cmux_stub_path,
            r#"#!/usr/bin/env python3
import json
import os
import subprocess
import sys

args = sys.argv[1:]
method = args[1]
params = json.loads(args[2])
raw_prompt = os.environ['RAW_PROMPT']
path = os.environ['CMUX_STUB_RECEIPT']
if method == 'workspace.create':
    with open(path, 'w') as handle:
        json.dump({
            'calls': [{
                'argv': args,
                'commandVerb': args[:2],
                'cwd': params['working_directory'],
                'focus': params.get('focus'),
                'eagerLoadTerminal': params.get('eager_load_terminal'),
                'hasInitialCommand': 'initial_command' in params,
                'rawPromptInArgv': raw_prompt in '\n'.join(args),
            }],
        }, handle, indent=2)
    print(json.dumps({'workspace_ref': 'workspace:stub', 'workspace_id': 'WORKSPACE-STUB'}))
    sys.exit(0)
if method == 'surface.create':
    cwd = params['working_directory']
    command = params['initial_command']
    result = subprocess.run(command, cwd=cwd, shell=True, executable='/bin/zsh')
    try:
        with open(path) as handle:
            receipt = json.load(handle)
    except FileNotFoundError:
        receipt = {'calls': []}
    receipt['calls'].append({
        'argv': args,
        'commandVerb': args[:2],
        'workspace': params.get('workspace_id'),
        'cwd': cwd,
        'focus': params.get('focus'),
        'tmuxStartMatchesInitial': params.get('tmux_start_command') == command,
        'rawPromptInArgv': raw_prompt in '\n'.join(args),
        'rawPromptInCommand': raw_prompt in command,
        'wrapperExitCode': result.returncode,
    })
    with open(path, 'w') as handle:
        json.dump(receipt, handle, indent=2)
    sys.exit(result.returncode)
raise SystemExit(f'unexpected method: {method}')
"#,
        )
        .expect("write cmux stub");
        std::fs::write(
            &codex_stub_path,
            r#"#!/usr/bin/env python3
import hashlib
import json
import os
import sys

prompt = sys.argv[4]
with open(os.environ['CODEX_STUB_RECEIPT'], 'w') as handle:
    json.dump({
        'pwd': os.getcwd(),
        'argv': sys.argv[1:],
        'promptChars': len(prompt),
        'promptSha256': hashlib.sha256(prompt.encode()).hexdigest(),
        'promptRepr': repr(prompt),
    }, handle, indent=2)
"#,
        )
        .expect("write codex stub");
        set_file_mode(&cmux_stub_path, 0o700).expect("chmod cmux stub");
        set_file_mode(&codex_stub_path, 0o700).expect("chmod codex stub");

        let _env_guard = HandoffEnvGuard::set([
            (DRY_RUN_ENV, None),
            (
                RECEIPT_PATH_ENV,
                Some(handoff_receipt_path.to_string_lossy().to_string()),
            ),
            (
                CMUX_BINARY_ENV,
                Some(cmux_stub_path.to_string_lossy().to_string()),
            ),
            (
                CODEX_BINARY_ENV,
                Some(codex_stub_path.to_string_lossy().to_string()),
            ),
            (
                "CMUX_STUB_RECEIPT",
                Some(cmux_receipt_path.to_string_lossy().to_string()),
            ),
            (
                "CODEX_STUB_RECEIPT",
                Some(codex_receipt_path.to_string_lossy().to_string()),
            ),
            ("RAW_PROMPT", Some(prompt.to_string())),
        ]);

        let payload = AgentPromptHandoffPayload {
            source: AgentPromptHandoffSource::AgentChatComposer,
            adapter_id: AgentPromptHandoffAdapterId::CmuxCodex,
            raw_input: prompt.to_string(),
            prompt: prompt.to_string(),
            cwd: project_dir.clone(),
            model_id: Some("gpt-5.1-codex".to_string()),
            profile_id: Some("script-kit".to_string()),
            context_part_count: 2,
            prompt_builder_segment_count: 3,
            warnings: Vec::new(),
        };
        let receipt = launch_prompt_handoff(&payload).expect("launch handoff");
        assert!(!receipt.dry_run);
        assert!(receipt.spawned);
        assert!(receipt.pid.is_some());
        assert!(receipt.prompt_file_created);
        assert!(receipt.script_file_created);
        assert_eq!(
            receipt.command_kind,
            "cmux_workspace_surface_create_initial_command"
        );
        assert_eq!(receipt.prompt_sha256, sha256_hex(prompt));

        wait_for_file(&codex_receipt_path, Duration::from_secs(5)).expect("codex receipt");
        wait_for_file_containing(
            &cmux_receipt_path,
            "\"surface.create\"",
            Duration::from_secs(5),
        )
        .expect("cmux receipt");
        wait_for_file(&handoff_receipt_path, Duration::from_secs(5)).expect("handoff receipt");

        let cmux_receipt = std::fs::read_to_string(&cmux_receipt_path).expect("cmux receipt");
        assert!(cmux_receipt.contains("\"workspace.create\""));
        assert!(cmux_receipt.contains("\"surface.create\""));
        assert!(cmux_receipt.contains(&format!("\"cwd\": \"{}\"", project_dir.to_string_lossy())));
        assert!(cmux_receipt.contains("\"focus\": true"));
        assert!(cmux_receipt.contains("\"eagerLoadTerminal\": true"));
        assert!(cmux_receipt.contains("\"hasInitialCommand\": false"));
        assert!(cmux_receipt.contains("\"workspace\": \"workspace:stub\""));
        assert!(cmux_receipt.contains("\"tmuxStartMatchesInitial\": true"));
        assert!(cmux_receipt.contains("\"rawPromptInArgv\": false"));
        assert!(cmux_receipt.contains("\"rawPromptInCommand\": false"));
        assert!(cmux_receipt.contains("\"wrapperExitCode\": 0"));

        let codex_receipt = std::fs::read_to_string(&codex_receipt_path).expect("codex receipt");
        let canonical_project_dir =
            std::fs::canonicalize(&project_dir).expect("canonical project dir");
        assert!(codex_receipt.contains(&format!(
            "\"pwd\": \"{}\"",
            canonical_project_dir.to_string_lossy()
        )));
        assert!(codex_receipt.contains("\"--cd\""));
        assert!(codex_receipt.contains(&format!("\"{}\"", project_dir.to_string_lossy())));
        assert!(codex_receipt.contains("\"--\""));
        assert!(codex_receipt.contains(&format!("\"promptChars\": {}", prompt.chars().count())));
        assert!(codex_receipt.contains(&format!("\"promptSha256\": \"{}\"", sha256_hex(prompt))));
        assert!(codex_receipt.contains("trailing newline\\\\n'"));

        let handoff_receipt =
            std::fs::read_to_string(&handoff_receipt_path).expect("handoff receipt");
        assert!(handoff_receipt.contains("\"spawned\": true"));
        assert!(handoff_receipt.contains(&format!("\"promptSha256\": \"{}\"", sha256_hex(prompt))));
        assert!(!handoff_receipt.contains(prompt));
    }

    #[test]
    fn cmux_codex_rejects_nul_prompt_before_launch() {
        let payload = AgentPromptHandoffPayload {
            source: AgentPromptHandoffSource::AgentChatComposer,
            adapter_id: AgentPromptHandoffAdapterId::CmuxCodex,
            raw_input: "contains nul".to_string(),
            prompt: "contains\0nul".to_string(),
            cwd: PathBuf::from("/tmp"),
            model_id: None,
            profile_id: None,
            context_part_count: 0,
            prompt_builder_segment_count: 0,
            warnings: Vec::new(),
        };

        assert!(matches!(
            launch_prompt_handoff(&payload),
            Err(AgentPromptHandoffError::UnsupportedPrompt(reason))
                if reason.contains("NUL bytes")
        ));
    }

    struct RichPromptContextFixture {
        temp_dir: tempfile::TempDir,
        raw_prompt: String,
        parts: Vec<AiContextPart>,
        expected_fragments: Vec<String>,
    }

    impl RichPromptContextFixture {
        fn new() -> Self {
            let temp_dir = tempfile::tempdir().expect("temp dir");
            let text_file_path = temp_dir.path().join("brief.txt");
            let image_file_path = temp_dir.path().join("screenshot.png");
            let skill_path = temp_dir.path().join("SKILL.md");

            std::fs::write(
                &text_file_path,
                "PROMPT_EXPORT_FILE_REFERENCE_SENTINEL\nUse this local file.",
            )
            .expect("write text file");
            std::fs::write(&image_file_path, [0x89, b'P', b'N', b'G', 0xff])
                .expect("write binary screenshot");
            std::fs::write(
                &skill_path,
                "# Prompt Export Skill\nPROMPT_EXPORT_SKILL_FILE_SENTINEL",
            )
            .expect("write skill");

            let focused_target = crate::ai::tab_context::TabAiTargetContext {
                source: "ClipboardHistory".to_string(),
                kind: "clipboard_entry".to_string(),
                semantic_id: "clipboard-entry:PROMPT_EXPORT_FOCUSED_TARGET_SENTINEL".to_string(),
                label: "Focused clipboard entry".to_string(),
                metadata: Some(serde_json::json!({
                    "preview": "PROMPT_EXPORT_FOCUSED_TARGET_METADATA_SENTINEL",
                    "contentType": "text",
                })),
            };

            let parts = vec![
                AiContextPart::ResourceUri {
                    uri: "kit://context/schema".to_string(),
                    label: "Context Schema".to_string(),
                },
                AiContextPart::FilePath {
                    path: text_file_path.to_string_lossy().to_string(),
                    label: "brief.txt".to_string(),
                },
                AiContextPart::FilePath {
                    path: image_file_path.to_string_lossy().to_string(),
                    label: "screenshot.png".to_string(),
                },
                AiContextPart::SkillFile {
                    path: skill_path.to_string_lossy().to_string(),
                    label: "/prompt-export-skill".to_string(),
                    skill_name: "Prompt Export Skill".to_string(),
                    owner_label: "Script Kit Test".to_string(),
                    slash_name: "prompt-export-skill".to_string(),
                },
                AiContextPart::FocusedTarget {
                    target: focused_target,
                    label: "Focused clipboard entry".to_string(),
                },
                AiContextPart::AmbientContext {
                    label: "PROMPT_EXPORT_AMBIENT_DISPLAY_ONLY_SENTINEL".to_string(),
                },
                AiContextPart::TextBlock {
                    label: "Clipboard history text".to_string(),
                    source: "clipboard-history://entry/PROMPT_EXPORT_CLIPBOARD_SOURCE_SENTINEL"
                        .to_string(),
                    text: "PROMPT_EXPORT_CLIPBOARD_HISTORY_SENTINEL".to_string(),
                    mime_type: Some("text/plain".to_string()),
                },
                AiContextPart::TextBlock {
                    label: "Browser tab text".to_string(),
                    source: "browser-tab://PROMPT_EXPORT_BROWSER_TAB_SOURCE_SENTINEL".to_string(),
                    text: "PROMPT_EXPORT_BROWSER_TAB_SENTINEL".to_string(),
                    mime_type: Some("text/uri-list".to_string()),
                },
            ];

            let expected_fragments = vec![
                "kit://context/schema".to_string(),
                "PROMPT_EXPORT_FILE_REFERENCE_SENTINEL".to_string(),
                image_file_path.to_string_lossy().to_string(),
                "unreadable=\"true\"".to_string(),
                "PROMPT_EXPORT_SKILL_FILE_SENTINEL".to_string(),
                "PROMPT_EXPORT_FOCUSED_TARGET_SENTINEL".to_string(),
                "PROMPT_EXPORT_FOCUSED_TARGET_METADATA_SENTINEL".to_string(),
                "PROMPT_EXPORT_CLIPBOARD_HISTORY_SENTINEL".to_string(),
                "PROMPT_EXPORT_BROWSER_TAB_SENTINEL".to_string(),
                "Rewrite the prompt export proof with every context source".to_string(),
            ];

            Self {
                temp_dir,
                raw_prompt: "/rewrite Rewrite the prompt export proof with every context source"
                    .to_string(),
                parts,
                expected_fragments,
            }
        }
    }

    fn compile_fixture_payload(fixture: &RichPromptContextFixture) -> AgentPromptHandoffPayload {
        let parse = crate::spine::parse_spine(&fixture.raw_prompt);
        let plan = crate::spine::prompt_plan::build_spine_prompt_plan(&parse);
        assert!(plan.should_submit_to_chat(), "fixture prompt should submit");

        compile_handoff_payload_from_spine_plan(
            AgentPromptHandoffAdapterId::CmuxCodex,
            fixture.raw_prompt.clone(),
            fixture.temp_dir.path().to_path_buf(),
            Some("gpt-test".to_string()),
            fixture.parts.clone(),
            plan,
        )
        .expect("compile rich fixture")
    }

    fn assert_compiled_prompt_contains_all_context_fingerprints(
        prompt: &str,
        fixture: &RichPromptContextFixture,
    ) {
        for fragment in &fixture.expected_fragments {
            assert!(
                prompt.contains(fragment),
                "compiled prompt missing {fragment:?}: {prompt}"
            );
        }
    }

    fn assert_export_receipt_matches_payload(
        receipt: &AgentPromptExportReceipt,
        payload: &AgentPromptHandoffPayload,
        export_kind: &str,
        command_kind: &str,
    ) {
        assert_eq!(receipt.export_kind, export_kind);
        assert_eq!(receipt.command_kind, command_kind);
        assert_eq!(receipt.prompt_sha256, sha256_hex(&payload.prompt));
        assert_eq!(receipt.prompt_chars, payload.prompt.chars().count());
        assert_eq!(receipt.context_part_count, payload.context_part_count);
        assert_eq!(
            receipt.prompt_builder_segment_count,
            payload.prompt_builder_segment_count
        );
    }

    fn assert_ai_context_part_variant_coverage(parts: &[AiContextPart]) {
        let mut names = parts
            .iter()
            .map(ai_context_part_variant_name)
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        assert_eq!(
            names,
            vec![
                "ambientContext",
                "filePath",
                "focusedTarget",
                "resourceUri",
                "skillFile",
                "textBlock"
            ]
        );
    }

    fn ai_context_part_variant_name(part: &AiContextPart) -> &'static str {
        match part {
            AiContextPart::ResourceUri { .. } => "resourceUri",
            AiContextPart::FilePath { .. } => "filePath",
            AiContextPart::SkillFile { .. } => "skillFile",
            AiContextPart::FocusedTarget { .. } => "focusedTarget",
            AiContextPart::AmbientContext { .. } => "ambientContext",
            AiContextPart::TextBlock { .. } => "textBlock",
        }
    }

    fn wait_for_file(path: &Path, timeout: Duration) -> Result<(), String> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if path.exists() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err(format!("timed out waiting for {}", path.display()))
    }

    fn wait_for_file_containing(
        path: &Path,
        expected: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if let Ok(contents) = std::fs::read_to_string(path) {
                if contents.contains(expected) {
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err(format!(
            "timed out waiting for {} to contain {expected:?}",
            path.display()
        ))
    }

    fn test_payload(prompt: &str) -> AgentPromptHandoffPayload {
        AgentPromptHandoffPayload {
            source: AgentPromptHandoffSource::AgentChatComposer,
            adapter_id: AgentPromptHandoffAdapterId::CmuxCodex,
            raw_input: prompt.to_string(),
            prompt: prompt.to_string(),
            cwd: PathBuf::from("/tmp/script-kit-prompt-export-test"),
            model_id: Some("gpt-test".to_string()),
            profile_id: None,
            context_part_count: 0,
            prompt_builder_segment_count: 0,
            warnings: Vec::new(),
        }
    }

    struct HandoffEnvGuard(Vec<(&'static str, Option<String>)>);

    impl HandoffEnvGuard {
        fn set<const N: usize>(values: [(&'static str, Option<String>); N]) -> Self {
            let mut previous = Vec::with_capacity(N);
            for (name, value) in values {
                previous.push((name, std::env::var(name).ok()));
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
            Self(previous)
        }
    }

    impl Drop for HandoffEnvGuard {
        fn drop(&mut self) {
            restore_handoff_env(std::mem::take(&mut self.0));
        }
    }

    fn restore_handoff_env(previous: Vec<(&'static str, Option<String>)>) {
        for (name, value) in previous {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

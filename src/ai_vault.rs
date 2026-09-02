#![allow(dead_code)]

use anyhow::{anyhow, Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const FAST_CODEX_ROW_LIMIT: usize = 500;
const WARM_CODEX_ROW_LIMIT: usize = 2_000;
const SYNC_CONTENT_SCAN_LIMIT: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct RootAiVaultSectionOptions {
    pub enabled: bool,
    pub max_results: usize,
    pub min_query_chars: usize,
    pub providers: Vec<crate::config::AiVaultProvider>,
    pub cache_ttl_ms: u64,
    pub search_content: bool,
    pub exclude_patterns: Vec<String>,
}

impl Default for RootAiVaultSectionOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            max_results: crate::config::defaults::DEFAULT_UNIFIED_SEARCH_AI_VAULT_MAX_RESULTS,
            min_query_chars:
                crate::config::defaults::DEFAULT_UNIFIED_SEARCH_AI_VAULT_MIN_QUERY_CHARS,
            providers: crate::config::AiVaultProvider::default_root_providers(),
            cache_ttl_ms: crate::config::defaults::DEFAULT_UNIFIED_SEARCH_AI_VAULT_CACHE_TTL_MS,
            search_content: crate::config::defaults::DEFAULT_UNIFIED_SEARCH_AI_VAULT_SEARCH_CONTENT,
            exclude_patterns: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AiVaultMatchedField {
    Title,
    Transcript,
    Model,
    Workspace,
    Provider,
    #[default]
    Recent,
}

impl AiVaultMatchedField {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Title => "Matched title",
            Self::Transcript => "Matched transcript",
            Self::Model => "Matched model",
            Self::Workspace => "Matched workspace",
            Self::Provider => "Matched provider",
            Self::Recent => "Recent session",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiVaultHit {
    pub provider: String,
    #[serde(default)]
    pub provider_display_name: String,
    pub session_id: String,
    pub source_kind: Option<String>,
    #[serde(default)]
    pub safe_title: String,
    pub workspace_path: Option<String>,
    pub model: Option<String>,
    pub modified_at: Option<String>,
    #[serde(default)]
    pub matched_field: AiVaultMatchedField,
    #[serde(default)]
    pub stable_key: String,
    #[serde(default)]
    pub score: i32,
    #[serde(skip, default)]
    search_terms: Vec<String>,
    #[serde(skip, default)]
    search_haystack: String,
    #[serde(skip, default)]
    rollout_path: Option<PathBuf>,
}

impl AiVaultHit {
    fn metadata_search_terms(&self) -> Vec<String> {
        let mut terms = vec![
            self.safe_title.clone(),
            self.provider.clone(),
            self.provider_display_name.clone(),
            self.session_id.clone(),
        ];
        if let Some(source_kind) = self.source_kind.as_ref() {
            terms.push(source_kind.clone());
        }
        if let Some(model) = self.model.as_ref() {
            terms.push(model.clone());
        }
        if let Some(workspace) = self.workspace_path.as_ref() {
            terms.push(workspace.clone());
        }
        terms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AiVaultTerminalRouting {
    UserPreferred,
    NewTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiVaultResumeReceipt {
    pub status: String,
    pub provider: String,
    pub session_id: String,
    pub terminal_routing: String,
    pub terminal_target_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AiVaultSnapshotStatus {
    pub generation: u64,
}

pub(crate) fn root_ai_vault_snapshot_status() -> AiVaultSnapshotStatus {
    AiVaultSnapshotStatus {
        generation: ai_vault_cache_generation().load(Ordering::Relaxed),
    }
}

pub(crate) fn root_ai_vault_query_is_eligible(
    query: &str,
    options: &RootAiVaultSectionOptions,
) -> bool {
    let query = query.trim();
    options.enabled
        && (query.is_empty()
            || crate::scripts::search::query_meets_min_query_chars(query, options.min_query_chars))
}

pub(crate) fn search_root_ai_vault_direct(
    query: &str,
    options: RootAiVaultSectionOptions,
) -> Vec<AiVaultHit> {
    let (mut hits, fixture_backed) = match load_fixture_hits() {
        Ok(Some(hits)) => (hits, true),
        Ok(None) => {
            let hits = search_local_vault(query, options.clone()).unwrap_or_else(|_error| {
                tracing::warn!(
                    target: "script_kit::ai_vault",
                    event = "ai_vault_local_search_unavailable",
                    error_kind = "other",
                    "AI Vault search failed"
                );
                Vec::new()
            });
            (hits, false)
        }
        Err(_error) => {
            tracing::warn!(
                target: "script_kit::ai_vault",
                event = "ai_vault_fixture_unavailable",
                error_kind = "other",
                "AI Vault fixture search failed"
            );
            (Vec::new(), true)
        }
    };

    for hit in &mut hits {
        normalize_hit(hit);
    }

    apply_ai_vault_exclusions(&mut hits, &options);

    let normalized_query = query.trim().to_ascii_lowercase();
    if fixture_backed && !normalized_query.is_empty() {
        hits.retain(|hit| hit_matches_query(hit, &normalized_query, options.search_content));
    }
    hits.truncate(options.max_results);
    hits
}

pub(crate) fn resume_vault_session(
    hit: &AiVaultHit,
    terminal_routing: AiVaultTerminalRouting,
) -> AiVaultResumeReceipt {
    if let Some(receipt) = resume_local_vault_session(hit, terminal_routing) {
        return receipt;
    }

    let request = serde_json::json!({
        "type": "aiVault.resume.v1",
        "provider": hit.provider,
        "sessionId": hit.session_id,
        "sourceKind": hit.source_kind,
        "workspacePath": hit.workspace_path,
        "terminalRouting": terminal_routing_value(terminal_routing),
    });

    let Some(mut command) = cmux_command() else {
        return AiVaultResumeReceipt {
            status: "unavailable".to_string(),
            provider: hit.provider.clone(),
            session_id: hit.session_id.clone(),
            terminal_routing: terminal_routing_value(terminal_routing).to_string(),
            terminal_target_id: None,
            error: Some("cmux command not configured".to_string()),
        };
    };

    command
        .arg("ai-vault")
        .arg("resume")
        .arg("--json")
        .arg(request.to_string());
    match output_with_timeout(command, Duration::from_millis(5_000)) {
        Ok(output) if output.status.success() => {
            serde_json::from_slice::<AiVaultResumeReceipt>(&output.stdout)
                .unwrap_or_else(|_| launched_receipt(hit, terminal_routing, "launched"))
        }
        Ok(output) => AiVaultResumeReceipt {
            status: "error".to_string(),
            provider: hit.provider.clone(),
            session_id: hit.session_id.clone(),
            terminal_routing: terminal_routing_value(terminal_routing).to_string(),
            terminal_target_id: None,
            error: Some(cmux_failure_message("resume", output.status.code())),
        },
        Err(_error) => AiVaultResumeReceipt {
            status: "error".to_string(),
            provider: hit.provider.clone(),
            session_id: hit.session_id.clone(),
            terminal_routing: terminal_routing_value(terminal_routing).to_string(),
            terminal_target_id: None,
            error: Some(cmux_failure_message("resume", None)),
        },
    }
}

pub(crate) fn resume_command_for_hit(
    hit: &AiVaultHit,
    terminal_routing: AiVaultTerminalRouting,
) -> String {
    if let Some(resume_command) = local_resume_command(hit) {
        return resume_command;
    }

    let request = resume_request_json(hit, terminal_routing);
    format!(
        "{} ai-vault resume --json {}",
        shell_quote(&cmux_binary_name()),
        shell_quote(&request.to_string())
    )
}

fn resume_local_vault_session(
    hit: &AiVaultHit,
    terminal_routing: AiVaultTerminalRouting,
) -> Option<AiVaultResumeReceipt> {
    let resume_command = local_resume_command(hit)?;
    let Some(mut command) = cmux_command() else {
        return Some(AiVaultResumeReceipt {
            status: "unavailable".to_string(),
            provider: hit.provider.clone(),
            session_id: hit.session_id.clone(),
            terminal_routing: terminal_routing_value(terminal_routing).to_string(),
            terminal_target_id: None,
            error: Some("cmux command not configured".to_string()),
        });
    };

    let cwd = hit
        .workspace_path
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| home_dir().to_string_lossy().to_string());
    command
        .arg("new-workspace")
        .arg("--name")
        .arg(format!("AI Vault: {}", short_title(&hit.safe_title, 80)))
        .arg("--description")
        .arg(format!(
            "{} resume from Script Kit AI Vault",
            human_provider(&hit.provider)
        ))
        .arg("--cwd")
        .arg(cwd)
        .arg("--command")
        .arg(resume_command)
        .arg("--focus")
        .arg("true");

    match output_with_timeout(command, Duration::from_millis(5_000)) {
        Ok(output) if output.status.success() => Some(AiVaultResumeReceipt {
            status: "launched".to_string(),
            provider: hit.provider.clone(),
            session_id: hit.session_id.clone(),
            terminal_routing: terminal_routing_value(terminal_routing).to_string(),
            terminal_target_id: parse_cmux_target_id(&output.stdout),
            error: None,
        }),
        Ok(output) => Some(AiVaultResumeReceipt {
            status: "error".to_string(),
            provider: hit.provider.clone(),
            session_id: hit.session_id.clone(),
            terminal_routing: terminal_routing_value(terminal_routing).to_string(),
            terminal_target_id: None,
            error: Some(cmux_failure_message("new-workspace", output.status.code())),
        }),
        Err(_error) => Some(AiVaultResumeReceipt {
            status: "error".to_string(),
            provider: hit.provider.clone(),
            session_id: hit.session_id.clone(),
            terminal_routing: terminal_routing_value(terminal_routing).to_string(),
            terminal_target_id: None,
            error: Some(cmux_failure_message("new-workspace", None)),
        }),
    }
}

fn resume_request_json(
    hit: &AiVaultHit,
    terminal_routing: AiVaultTerminalRouting,
) -> serde_json::Value {
    serde_json::json!({
        "type": "aiVault.resume.v1",
        "provider": hit.provider,
        "sessionId": hit.session_id,
        "sourceKind": hit.source_kind,
        "workspacePath": hit.workspace_path,
        "terminalRouting": terminal_routing_value(terminal_routing),
    })
}

fn local_resume_command(hit: &AiVaultHit) -> Option<String> {
    match hit.provider.as_str() {
        "claude" => {
            let config_dir = std::env::var("CLAUDE_CONFIG_DIR")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| home_dir().join(".claude").to_string_lossy().to_string());
            Some(format!(
                "env CLAUDE_CONFIG_DIR={} claude --resume {}",
                shell_quote(&config_dir),
                shell_quote(&hit.session_id)
            ))
        }
        "codex" => Some(format!("codex resume {}", shell_quote(&hit.session_id))),
        _ => None,
    }
}

pub(crate) fn reveal_vault_session(hit: &AiVaultHit) -> AiVaultResumeReceipt {
    let Some(mut command) = cmux_command() else {
        return AiVaultResumeReceipt {
            status: "error".to_string(),
            provider: hit.provider.clone(),
            session_id: hit.session_id.clone(),
            terminal_routing: "reveal".to_string(),
            terminal_target_id: None,
            error: Some("cmux command unavailable".to_string()),
        };
    };

    let request = serde_json::json!({
        "type": "aiVault.reveal.v1",
        "provider": hit.provider,
        "sessionId": hit.session_id,
        "sourceKind": hit.source_kind,
        "workspacePath": hit.workspace_path,
    });
    command
        .arg("ai-vault")
        .arg("reveal")
        .arg("--json")
        .arg(request.to_string());
    let output = output_with_timeout(command, Duration::from_millis(5_000));
    match output {
        Ok(output) if output.status.success() => {
            serde_json::from_slice::<AiVaultResumeReceipt>(&output.stdout).unwrap_or_else(|_| {
                launched_receipt(hit, AiVaultTerminalRouting::UserPreferred, "opened")
            })
        }
        Ok(output) => AiVaultResumeReceipt {
            status: "error".to_string(),
            provider: hit.provider.clone(),
            session_id: hit.session_id.clone(),
            terminal_routing: "reveal".to_string(),
            terminal_target_id: None,
            error: Some(cmux_failure_message("reveal", output.status.code())),
        },
        Err(_error) => AiVaultResumeReceipt {
            status: "error".to_string(),
            provider: hit.provider.clone(),
            session_id: hit.session_id.clone(),
            terminal_routing: "reveal".to_string(),
            terminal_target_id: None,
            error: Some(cmux_failure_message("reveal", None)),
        },
    }
}

fn search_cmux_vault(query: &str, options: RootAiVaultSectionOptions) -> Result<Vec<AiVaultHit>> {
    let cache_key = ai_vault_cache_key(query, &options);
    if let Some(hits) = ai_vault_cache_get(&cache_key, options.cache_ttl_ms) {
        return Ok(hits);
    }

    let Some(mut command) = cmux_command() else {
        return Ok(Vec::new());
    };
    let providers = options
        .providers
        .iter()
        .map(crate::config::AiVaultProvider::cmux_id)
        .collect::<Vec<_>>();
    let request = serde_json::json!({
        "type": "aiVault.search.v1",
        "query": query,
        "limit": cmux_search_limit(&options),
        "offset": 0,
        "providers": providers,
        "cwdFilter": serde_json::Value::Null,
        "includeContent": false,
    });
    command
        .arg("ai-vault")
        .arg("search")
        .arg("--json")
        .arg(request.to_string());
    let output = output_with_timeout(
        command,
        Duration::from_millis(options.cache_ttl_ms.min(5_000)),
    )
    .context("run cmux ai-vault search")?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SearchResponse {
        hits: Vec<AiVaultHit>,
    }
    match serde_json::from_slice::<SearchResponse>(&output.stdout) {
        Ok(response) => {
            let mut hits = response.hits;
            for hit in &mut hits {
                normalize_hit(hit);
            }
            apply_ai_vault_exclusions(&mut hits, &options);
            hits.truncate(options.max_results);
            ai_vault_cache_put(cache_key, hits.clone());
            Ok(hits)
        }
        Err(_error) => {
            tracing::warn!(
                target: "script_kit::ai_vault",
                event = "ai_vault_cmux_response_parse_failed",
                error_kind = "json",
                "AI Vault cmux response parse failed"
            );
            Ok(Vec::new())
        }
    }
}

fn search_local_vault(query: &str, options: RootAiVaultSectionOptions) -> Result<Vec<AiVaultHit>> {
    let normalized_query = query.trim().to_ascii_lowercase();
    let mut hits = local_vault_index(options.clone())?;
    apply_ai_vault_exclusions(&mut hits, &options);
    if !normalized_query.is_empty() {
        let mut matched_hits = Vec::with_capacity(options.max_results);
        for mut hit in hits {
            if apply_local_vault_query_match(&mut hit, &normalized_query, options.search_content) {
                matched_hits.push(hit);
                if matched_hits.len() >= options.max_results {
                    break;
                }
            }
        }
        hits = matched_hits;
    } else {
        for hit in &mut hits {
            hit.matched_field = AiVaultMatchedField::Recent;
        }
    }
    hits.truncate(options.max_results);
    Ok(hits)
}

fn local_vault_index(options: RootAiVaultSectionOptions) -> Result<Vec<AiVaultHit>> {
    let cache_key = ai_vault_index_cache_key(&options);
    if let Some(hits) = ai_vault_cache_get(&cache_key, options.cache_ttl_ms) {
        return Ok(hits);
    }

    let hits = build_local_vault_index(&options, AiVaultIndexMode::Fast)?;
    ai_vault_cache_put(cache_key.clone(), hits.clone());
    spawn_warm_ai_vault_index(options, cache_key);
    Ok(hits)
}

#[derive(Clone, Copy)]
enum AiVaultIndexMode {
    Fast,
    Warm,
}

fn build_local_vault_index(
    options: &RootAiVaultSectionOptions,
    mode: AiVaultIndexMode,
) -> Result<Vec<AiVaultHit>> {
    let mut hits = Vec::new();
    if matches!(mode, AiVaultIndexMode::Warm) && provider_enabled(options, "claude") {
        hits.extend(read_claude_vault_hits()?);
    }
    if provider_enabled(options, "codex") {
        hits.extend(read_codex_vault_hits(options, codex_row_limit(mode))?);
    }
    hits.sort_by(|a, b| {
        b.modified_at
            .cmp(&a.modified_at)
            .then_with(|| a.stable_key.cmp(&b.stable_key))
    });
    for hit in &mut hits {
        refresh_search_haystack(hit);
    }
    Ok(hits)
}

fn codex_row_limit(mode: AiVaultIndexMode) -> usize {
    match mode {
        AiVaultIndexMode::Fast => FAST_CODEX_ROW_LIMIT,
        AiVaultIndexMode::Warm => WARM_CODEX_ROW_LIMIT,
    }
}

fn spawn_warm_ai_vault_index(options: RootAiVaultSectionOptions, cache_key: String) {
    if warm_ai_vault_index_inflight_set(&cache_key) {
        return;
    }
    std::thread::spawn(move || {
        match build_local_vault_index(&options, AiVaultIndexMode::Warm) {
            Ok(hits) => ai_vault_cache_put(cache_key.clone(), hits),
            Err(error) => tracing::warn!(
                target: "script_kit::ai_vault",
                event = "ai_vault_warm_index_failed",
                error = %error,
                "AI Vault warm index failed"
            ),
        }
        warm_ai_vault_index_inflight_clear(&cache_key);
    });
}

fn warm_ai_vault_index_inflight_set(key: &str) -> bool {
    let Ok(mut inflight) = warm_ai_vault_index_inflight().lock() else {
        return true;
    };
    if inflight.iter().any(|candidate| candidate == key) {
        return true;
    }
    inflight.push(key.to_string());
    false
}

fn warm_ai_vault_index_inflight_clear(key: &str) {
    if let Ok(mut inflight) = warm_ai_vault_index_inflight().lock() {
        inflight.retain(|candidate| candidate != key);
    }
}

fn warm_ai_vault_index_inflight() -> &'static Mutex<Vec<String>> {
    static INFLIGHT: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    INFLIGHT.get_or_init(|| Mutex::new(Vec::new()))
}

fn read_claude_vault_hits() -> Result<Vec<AiVaultHit>> {
    let root = home_dir().join(".claude").join("projects");
    let mut files = Vec::new();
    collect_jsonl_files(&root, &mut files);
    files.sort_by_key(|a| std::cmp::Reverse(a.1));
    files.truncate(300);

    let mut hits = Vec::new();
    for (path, mtime) in files {
        if let Some(hit) = read_claude_vault_hit(&path, mtime) {
            hits.push(hit);
        }
    }
    Ok(hits)
}

fn read_claude_vault_hit(path: &Path, mtime: SystemTime) -> Option<AiVaultHit> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let session_id = path.file_stem()?.to_string_lossy().to_string();
    let mut title = None;
    let mut cwd = None;
    let mut model = None;
    let mut newest = mtime;
    let mut search_terms = Vec::new();

    for line in reader.lines().map_while(Result::ok).take(1000) {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if let Some(timestamp) = event.get("timestamp").and_then(|value| value.as_str()) {
            if let Some(parsed) = parse_timestamp(timestamp) {
                newest = newest.max(parsed);
            }
        }
        if cwd.is_none() {
            cwd = event
                .get("cwd")
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string);
        }
        if model.is_none() {
            model = event
                .pointer("/message/model")
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string);
        }
        if title.is_some() || event.get("isMeta").and_then(|value| value.as_bool()) == Some(true) {
            continue;
        }
        let role = event
            .pointer("/message/role")
            .or_else(|| event.get("type"))
            .and_then(|value| value.as_str());
        if role != Some("user") {
            continue;
        }
        let content = event
            .pointer("/message/content")
            .or_else(|| event.get("content"));
        if let Some(content) = content {
            let mut fragments = Vec::new();
            collect_text_fragments(content, &mut fragments);
            if !fragments.is_empty() {
                search_terms.push(fragments.join("\n"));
            }
        }
        title = content.and_then(text_from_content_for_title);
    }

    let safe_title =
        title.unwrap_or_else(|| format!("Claude Code session {}", short_id(&session_id)));
    search_terms.extend([
        safe_title.clone(),
        session_id.clone(),
        cwd.clone().unwrap_or_default(),
        model.clone().unwrap_or_default(),
    ]);

    Some(AiVaultHit {
        provider: "claude".to_string(),
        provider_display_name: "Claude Code".to_string(),
        session_id: session_id.clone(),
        source_kind: Some("cli".to_string()),
        safe_title,
        workspace_path: cwd,
        model,
        modified_at: Some(system_time_to_rfc3339(newest)),
        matched_field: AiVaultMatchedField::Recent,
        stable_key: format!("ai-vault/claude/cli/{session_id}"),
        score: 0,
        search_terms,
        search_haystack: String::new(),
        rollout_path: Some(path.to_path_buf()),
    })
}

fn read_codex_vault_hits(
    options: &RootAiVaultSectionOptions,
    limit: usize,
) -> Result<Vec<AiVaultHit>> {
    let codex_dir = home_dir().join(".codex");
    let db_path = codex_dir.join("state_5.sqlite");
    let sessions_root = codex_dir.join("sessions");
    match read_codex_vault_hits_via_state_db(&db_path, &sessions_root, options, limit) {
        Ok(hits) => return Ok(hits),
        Err(error) => {
            let event = if db_path.exists() {
                "ai_vault_codex_state_db_unsupported"
            } else {
                "ai_vault_codex_state_db_unavailable"
            };
            tracing::warn!(
                target: "script_kit::ai_vault",
                event,
                db_path = %db_path.display(),
                error = %error,
                "Codex AI Vault state DB unavailable; falling back to session_index.jsonl"
            );
        }
    }
    read_codex_vault_hits_from_session_index()
}

fn read_codex_vault_hits_via_state_db(
    db_path: &Path,
    _sessions_root: &Path,
    _options: &RootAiVaultSectionOptions,
    limit: usize,
) -> Result<Vec<AiVaultHit>> {
    if !db_path.exists() {
        return Err(anyhow!("state_5.sqlite missing"));
    }
    let temp_dir = tempfile::tempdir().context("create temp dir for codex state snapshot")?;
    let copied_db = copy_sqlite_db_snapshot(db_path, temp_dir.path())?;
    let conn = Connection::open_with_flags(
        &copied_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("open_codex_state_db_failed: {}", copied_db.display()))?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, rollout_path, cwd, title, model, git_branch,
                   approval_mode, sandbox_policy, reasoning_effort,
                   first_user_message, updated_at_ms
            FROM threads
            WHERE COALESCE(archived, 0) = 0
            ORDER BY updated_at_ms DESC
            LIMIT ?1
            "#,
        )
        .context("codex_state_schema_unsupported")?;

    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(CodexThreadRow {
                session_id: row.get::<_, String>(0)?,
                rollout_path: row.get::<_, Option<String>>(1)?,
                cwd: row.get::<_, Option<String>>(2)?,
                title: row.get::<_, Option<String>>(3)?,
                model: row.get::<_, Option<String>>(4)?,
                git_branch: row.get::<_, Option<String>>(5)?,
                approval_mode: row.get::<_, Option<String>>(6)?,
                sandbox_policy: row.get::<_, Option<String>>(7)?,
                reasoning_effort: row.get::<_, Option<String>>(8)?,
                first_user_message: row.get::<_, Option<String>>(9)?,
                updated_at_ms: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
            })
        })
        .context("query_codex_state_threads")?;

    let mut hits = Vec::new();
    for row in rows {
        let row = row.context("read_codex_state_thread")?;
        if row.session_id.trim().is_empty() {
            continue;
        }
        hits.push(codex_hit_from_thread_row(row));
    }
    Ok(hits)
}

fn read_codex_vault_hits_from_session_index() -> Result<Vec<AiVaultHit>> {
    let index_path = home_dir().join(".codex").join("session_index.jsonl");
    let Ok(file) = File::open(index_path) else {
        return Ok(Vec::new());
    };
    let reader = BufReader::new(file);
    let mut hits = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        let Ok(row) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(session_id) = row
            .get("id")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
        else {
            continue;
        };
        let safe_title = row
            .get("thread_name")
            .and_then(|value| value.as_str())
            .and_then(normalize_title)
            .unwrap_or_else(|| format!("Codex session {}", short_id(session_id)));
        let cwd = row
            .get("cwd")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string);
        let modified = row
            .get("updated_at")
            .and_then(|value| value.as_str())
            .and_then(parse_timestamp)
            .unwrap_or(UNIX_EPOCH);
        hits.push(AiVaultHit {
            provider: "codex".to_string(),
            provider_display_name: "Codex".to_string(),
            session_id: session_id.to_string(),
            source_kind: Some("cli".to_string()),
            safe_title,
            workspace_path: cwd,
            model: row
                .get("model")
                .and_then(|value| value.as_str())
                .map(ToString::to_string),
            modified_at: Some(system_time_to_rfc3339(modified)),
            matched_field: AiVaultMatchedField::Recent,
            stable_key: format!("ai-vault/codex/cli/{session_id}"),
            score: 0,
            search_terms: Vec::new(),
            search_haystack: String::new(),
            rollout_path: None,
        });
    }
    hits.truncate(300);
    Ok(hits)
}

#[derive(Debug)]
struct CodexThreadRow {
    session_id: String,
    rollout_path: Option<String>,
    cwd: Option<String>,
    title: Option<String>,
    model: Option<String>,
    git_branch: Option<String>,
    approval_mode: Option<String>,
    sandbox_policy: Option<String>,
    reasoning_effort: Option<String>,
    first_user_message: Option<String>,
    updated_at_ms: i64,
}

fn codex_hit_from_thread_row(row: CodexThreadRow) -> AiVaultHit {
    let title = row
        .title
        .as_deref()
        .and_then(normalize_title)
        .or_else(|| {
            row.first_user_message
                .as_deref()
                .and_then(real_codex_user_message)
        })
        .unwrap_or_else(|| format!("Codex session {}", short_id(&row.session_id)));
    let modified = if row.updated_at_ms > 0 {
        UNIX_EPOCH + Duration::from_millis(row.updated_at_ms as u64)
    } else {
        UNIX_EPOCH
    };
    let rollout_path = row
        .rollout_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(expand_tilde_path);
    let mut search_terms = vec![
        title.clone(),
        row.session_id.clone(),
        row.rollout_path.clone().unwrap_or_default(),
        row.first_user_message.clone().unwrap_or_default(),
        row.git_branch.clone().unwrap_or_default(),
        row.approval_mode.clone().unwrap_or_default(),
        row.sandbox_policy.clone().unwrap_or_default(),
        row.reasoning_effort.clone().unwrap_or_default(),
    ];
    if let Some(cwd) = row.cwd.as_ref() {
        search_terms.push(cwd.clone());
    }
    if let Some(model) = row.model.as_ref() {
        search_terms.push(model.clone());
    }

    AiVaultHit {
        provider: "codex".to_string(),
        provider_display_name: "Codex".to_string(),
        session_id: row.session_id.clone(),
        source_kind: Some("cli".to_string()),
        safe_title: title,
        workspace_path: row.cwd.filter(|value| !value.trim().is_empty()),
        model: row.model.filter(|value| !value.trim().is_empty()),
        modified_at: Some(system_time_to_rfc3339(modified)),
        matched_field: AiVaultMatchedField::Recent,
        stable_key: format!("ai-vault/codex/cli/{}", row.session_id),
        score: 0,
        search_terms,
        search_haystack: String::new(),
        rollout_path,
    }
}

fn real_codex_user_message(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() || value.starts_with('<') {
        return None;
    }
    normalize_title(value)
}

fn hydrate_rollout_search_terms(hit: &mut AiVaultHit) {
    let Some(path) = hit.rollout_path.as_ref() else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let mut indexed = String::new();
    for line in text.lines().take(400) {
        if indexed.len() > 128 * 1024 {
            break;
        }
        indexed.push_str(line);
        indexed.push('\n');
    }
    if !indexed.is_empty() {
        hit.search_terms.push(indexed);
        hit.search_haystack.clear();
    }
}

fn copy_sqlite_db_snapshot(db_path: &Path, dest_root: &Path) -> Result<PathBuf> {
    let db_name = db_path
        .file_name()
        .ok_or_else(|| anyhow!("missing database filename"))?;
    let dest_db = dest_root.join(db_name);
    std::fs::copy(db_path, &dest_db)
        .with_context(|| format!("copy_codex_state_db_failed: {}", db_path.display()))?;

    for suffix in ["-wal", "-shm"] {
        let candidate = PathBuf::from(format!("{}{}", db_path.display(), suffix));
        if candidate.exists() {
            if let Some(candidate_name) = candidate.file_name() {
                let _ = std::fs::copy(&candidate, dest_root.join(candidate_name));
            }
        }
    }

    Ok(dest_db)
}

fn expand_tilde_path(value: &str) -> PathBuf {
    if value == "~" {
        return home_dir();
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    PathBuf::from(value)
}

fn collect_jsonl_files(root: &Path, files: &mut Vec<(PathBuf, SystemTime)>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            collect_jsonl_files(&path, files);
        } else if metadata.is_file()
            && path.extension().and_then(|ext| ext.to_str()) == Some("jsonl")
        {
            files.push((path, metadata.modified().unwrap_or(UNIX_EPOCH)));
        }
    }
}

fn text_from_content_for_title(content: &serde_json::Value) -> Option<String> {
    let mut fragments = Vec::new();
    collect_text_fragments(content, &mut fragments);
    normalize_title(&fragments.join("\n\n"))
}

fn collect_text_fragments(value: &serde_json::Value, fragments: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => fragments.push(text.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_text_fragments(item, fragments);
            }
        }
        serde_json::Value::Object(object) => {
            let value_type = object.get("type").and_then(|value| value.as_str());
            if matches!(
                value_type,
                Some("tool_use" | "tool_result" | "function_call" | "function_call_output")
            ) {
                return;
            }
            if let Some(text) = object.get("text").and_then(|value| value.as_str()) {
                fragments.push(text.to_string());
                return;
            }
            for key in ["content", "message"] {
                if let Some(child) = object.get(key) {
                    collect_text_fragments(child, fragments);
                    if !fragments.is_empty() {
                        return;
                    }
                }
            }
        }
        _ => {}
    }
}

fn normalize_title(raw: &str) -> Option<String> {
    let mut value = raw.replace(['\r', '\n', '\t'], " ");
    for tag in [
        "system-reminder",
        "command-name",
        "command-message",
        "command-args",
    ] {
        value = strip_tag(&value, tag);
    }
    value = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("[Image #1]", "")
        .trim()
        .to_string();
    if value.is_empty() {
        None
    } else if value.chars().count() > 160 {
        Some(format!(
            "{}...",
            value.chars().take(157).collect::<String>().trim_end()
        ))
    } else {
        Some(value)
    }
}

fn strip_tag(input: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    input.replace(&open, " ").replace(&close, " ")
}

fn matched_field(
    query: &str,
    title: &str,
    workspace: Option<&str>,
    provider: Option<&str>,
    session_id: &str,
) -> Option<AiVaultMatchedField> {
    if query.is_empty() {
        return Some(AiVaultMatchedField::Recent);
    }
    let query = query.to_ascii_lowercase();
    if title.to_ascii_lowercase().contains(&query) {
        return Some(AiVaultMatchedField::Title);
    }
    if workspace
        .unwrap_or("")
        .to_ascii_lowercase()
        .contains(&query)
    {
        return Some(AiVaultMatchedField::Workspace);
    }
    if provider.unwrap_or("").to_ascii_lowercase().contains(&query) {
        return Some(AiVaultMatchedField::Provider);
    }
    if session_id.to_ascii_lowercase().contains(&query) {
        return Some(AiVaultMatchedField::Recent);
    }
    None
}

fn apply_local_vault_query_match(hit: &mut AiVaultHit, query: &str, search_content: bool) -> bool {
    if query.is_empty() {
        hit.matched_field = AiVaultMatchedField::Recent;
        return true;
    }
    if title_matches_query(&hit.safe_title, query) {
        hit.matched_field = AiVaultMatchedField::Title;
        return true;
    }
    if hit
        .model
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase()
        .contains(query)
    {
        hit.matched_field = AiVaultMatchedField::Model;
        return true;
    }
    if hit
        .workspace_path
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase()
        .contains(query)
    {
        hit.matched_field = AiVaultMatchedField::Workspace;
        return true;
    }
    if hit
        .provider_display_name
        .to_ascii_lowercase()
        .contains(query)
        || hit.provider.to_ascii_lowercase().contains(query)
    {
        hit.matched_field = AiVaultMatchedField::Provider;
        return true;
    }
    if hit.session_id.to_ascii_lowercase().contains(query) {
        hit.matched_field = AiVaultMatchedField::Recent;
        return true;
    }
    if search_content && hit_search_haystack(hit).contains(query) {
        hit.matched_field = AiVaultMatchedField::Transcript;
        return true;
    }
    false
}

fn hit_search_haystack(hit: &mut AiVaultHit) -> &str {
    if hit.search_haystack.is_empty() {
        refresh_search_haystack(hit);
    }
    hit.search_haystack.as_str()
}

fn refresh_search_haystack(hit: &mut AiVaultHit) {
    let mut fields = hit.metadata_search_terms();
    fields.extend(hit.search_terms.clone());
    hit.search_haystack = fields.join("\u{1f}").to_ascii_lowercase();
}

fn append_bounded_content_matches(
    hits: &mut Vec<AiVaultHit>,
    query: &str,
    options: &RootAiVaultSectionOptions,
    remaining: usize,
) {
    if remaining == 0 || !provider_enabled(options, "codex") {
        return;
    }
    let Ok(mut candidates) = read_codex_vault_hits(options, SYNC_CONTENT_SCAN_LIMIT) else {
        return;
    };
    let existing = hits
        .iter()
        .map(|hit| hit.stable_key.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut added = 0;
    for mut candidate in candidates.drain(..) {
        if existing.contains(&candidate.stable_key) {
            continue;
        }
        hydrate_rollout_search_terms(&mut candidate);
        refresh_search_haystack(&mut candidate);
        if hit_search_haystack(&mut candidate).contains(query) {
            candidate.matched_field = AiVaultMatchedField::Transcript;
            hits.push(candidate);
            added += 1;
            if hits.len() >= options.max_results || added >= remaining {
                break;
            }
        }
    }
}

fn title_matches_query(title: &str, query: &str) -> bool {
    title.to_ascii_lowercase().contains(query)
}

fn parse_timestamp(timestamp: &str) -> Option<SystemTime> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| {
            UNIX_EPOCH
                + Duration::from_secs(value.timestamp().max(0) as u64)
                + Duration::from_nanos(value.timestamp_subsec_nanos() as u64)
        })
}

fn system_time_to_rfc3339(time: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time.into();
    datetime.to_rfc3339()
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn short_id(session_id: &str) -> String {
    session_id.chars().take(12).collect()
}

fn short_title(title: &str, max_chars: usize) -> String {
    let title = title.trim();
    if title.is_empty() {
        return "Conversation".to_string();
    }
    if title.chars().count() <= max_chars {
        title.to_string()
    } else {
        format!(
            "{}...",
            title
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
                .trim_end()
        )
    }
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn parse_cmux_target_id(stdout: &[u8]) -> Option<String> {
    let value = serde_json::from_slice::<serde_json::Value>(stdout).ok()?;
    for key in ["workspaceId", "workspace_id", "id", "url"] {
        if let Some(value) = value.get(key).and_then(|value| value.as_str()) {
            if !value.trim().is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn load_fixture_hits() -> Result<Option<Vec<AiVaultHit>>> {
    let Ok(value) = std::env::var("SCRIPT_KIT_AI_VAULT_TEST_PROVIDER") else {
        return Ok(None);
    };
    let raw = if Path::new(&value).exists() {
        std::fs::read_to_string(value).context("read SCRIPT_KIT_AI_VAULT_TEST_PROVIDER fixture")?
    } else {
        value
    };
    Ok(Some(
        serde_json::from_str::<Vec<AiVaultHit>>(&raw)
            .context("parse SCRIPT_KIT_AI_VAULT_TEST_PROVIDER fixture")?,
    ))
}

fn hit_matches_query(hit: &AiVaultHit, query: &str, search_content: bool) -> bool {
    let mut fields = hit.metadata_search_terms();
    if search_content {
        fields.extend(hit.search_terms.clone());
    }
    fields
        .into_iter()
        .any(|field| field.to_ascii_lowercase().contains(query))
}

fn normalize_hit(hit: &mut AiVaultHit) {
    if hit.provider_display_name.trim().is_empty() {
        hit.provider_display_name = human_provider(&hit.provider).to_string();
    }
    if hit.safe_title.trim().is_empty() {
        hit.safe_title = generic_title(hit);
    }
    if hit.stable_key.trim().is_empty() {
        let source = hit.source_kind.as_deref().unwrap_or("session");
        hit.stable_key = format!("ai-vault/{}/{}/{}", hit.provider, source, hit.session_id);
    }
}

fn generic_title(hit: &AiVaultHit) -> String {
    let short = hit.session_id.chars().take(8).collect::<String>();
    format!("{} session {}", human_provider(&hit.provider), short)
}

fn human_provider(provider: &str) -> &str {
    match provider {
        "claude" => "Claude Code",
        "codex" => "Codex",
        "hermes-agent" | "hermesAgent" => "Hermes Agent",
        "rovodev" | "rovoDev" => "Rovo Dev",
        _ => "AI Vault",
    }
}

fn cmux_command() -> Option<std::process::Command> {
    Some(std::process::Command::new(cmux_binary_name()))
}

fn cmux_binary_name() -> String {
    std::env::var("SCRIPT_KIT_CMUX_COMMAND")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "cmux".to_string())
}

fn output_with_timeout(
    mut command: std::process::Command,
    timeout: Duration,
) -> std::io::Result<Output> {
    let mut child = command.spawn()?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "cmux ai-vault command timed out",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn cmux_failure_message(action: &str, code: Option<i32>) -> String {
    match code {
        Some(code) => format!("cmux {action} failed with status {code}"),
        None => format!("cmux {action} failed"),
    }
}

fn ai_vault_cache_key(query: &str, options: &RootAiVaultSectionOptions) -> String {
    let providers = options
        .providers
        .iter()
        .map(crate::config::AiVaultProvider::cmux_id)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}exclude={}",
        query.trim(),
        options.max_results,
        options.search_content,
        providers,
        ai_vault_exclude_fingerprint(options)
    )
}

fn ai_vault_index_cache_key(options: &RootAiVaultSectionOptions) -> String {
    let providers = options
        .providers
        .iter()
        .map(crate::config::AiVaultProvider::cmux_id)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "local-index\u{1f}{providers}\u{1f}content={}\u{1f}{}",
        options.search_content,
        local_vault_fingerprint(&providers)
    )
}

fn local_vault_fingerprint(providers: &str) -> String {
    let mut parts = Vec::new();
    if providers.split(',').any(|provider| provider == "codex") {
        for path in codex_state_paths() {
            parts.push(path_fingerprint(&path));
        }
    }
    if providers.split(',').any(|provider| provider == "claude") {
        parts.push(path_fingerprint(
            &home_dir().join(".claude").join("projects"),
        ));
    }
    parts.join("|")
}

fn codex_state_paths() -> Vec<PathBuf> {
    let db = home_dir().join(".codex").join("state_5.sqlite");
    vec![
        db.clone(),
        PathBuf::from(format!("{}-wal", db.display())),
        PathBuf::from(format!("{}-shm", db.display())),
    ]
}

fn path_fingerprint(path: &Path) -> String {
    let Ok(metadata) = std::fs::metadata(path) else {
        return format!("{}:missing", path.display());
    };
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{}:{}:{}", path.display(), metadata.len(), modified)
}

fn provider_enabled(options: &RootAiVaultSectionOptions, provider: &str) -> bool {
    options
        .providers
        .iter()
        .any(|candidate| candidate.cmux_id() == provider)
}

fn cmux_search_limit(options: &RootAiVaultSectionOptions) -> usize {
    if options.exclude_patterns.is_empty() {
        return options.max_results;
    }
    options
        .max_results
        .saturating_mul(4)
        .max(options.max_results.saturating_add(20))
        .min(200)
}

fn ai_vault_exclude_fingerprint(options: &RootAiVaultSectionOptions) -> String {
    let joined = options
        .exclude_patterns
        .iter()
        .map(|pattern| normalize_ai_vault_pattern_value(pattern))
        .filter(|pattern| !pattern.is_empty())
        .collect::<Vec<_>>()
        .join("\u{1e}");
    format!("excl:{:016x}", stable_fnv1a64(joined.as_bytes()))
}

fn apply_ai_vault_exclusions(hits: &mut Vec<AiVaultHit>, options: &RootAiVaultSectionOptions) {
    if options.exclude_patterns.is_empty() {
        return;
    }
    hits.retain(|hit| !ai_vault_hit_excluded(hit, &options.exclude_patterns));
}

fn ai_vault_hit_excluded(hit: &AiVaultHit, patterns: &[String]) -> bool {
    patterns
        .iter()
        .map(|pattern| pattern.trim())
        .filter(|pattern| !pattern.is_empty())
        .any(|pattern| ai_vault_pattern_matches_hit(hit, pattern))
}

fn ai_vault_pattern_matches_hit(hit: &AiVaultHit, pattern: &str) -> bool {
    let (field, pattern) = match pattern.split_once(':') {
        Some((field, value)) if ai_vault_exclusion_field_known(field) => {
            (Some(field.trim().to_ascii_lowercase()), value.trim())
        }
        _ => (None, pattern),
    };
    if pattern.is_empty() {
        return false;
    }

    match field.as_deref() {
        Some("title") => ai_vault_pattern_matches_value(pattern, &hit.safe_title),
        Some("provider") => {
            ai_vault_pattern_matches_value(pattern, &hit.provider)
                || ai_vault_pattern_matches_value(pattern, &hit.provider_display_name)
        }
        Some("workspace") | Some("workspace_path") | Some("workspacepath") => hit
            .workspace_path
            .as_deref()
            .is_some_and(|value| ai_vault_pattern_matches_value(pattern, value)),
        Some("model") => hit
            .model
            .as_deref()
            .is_some_and(|value| ai_vault_pattern_matches_value(pattern, value)),
        Some("session") | Some("session_id") | Some("sessionid") => {
            ai_vault_pattern_matches_value(pattern, &hit.session_id)
        }
        Some("source") | Some("source_kind") | Some("sourcekind") => hit
            .source_kind
            .as_deref()
            .is_some_and(|value| ai_vault_pattern_matches_value(pattern, value)),
        Some("key") | Some("stable_key") | Some("stablekey") => {
            ai_vault_pattern_matches_value(pattern, &hit.stable_key)
        }
        Some(_) => ai_vault_pattern_matches_metadata(hit, pattern),
        None => ai_vault_pattern_matches_metadata(hit, pattern),
    }
}

fn ai_vault_exclusion_field_known(field: &str) -> bool {
    matches!(
        field.trim().to_ascii_lowercase().as_str(),
        "title"
            | "provider"
            | "workspace"
            | "workspace_path"
            | "workspacepath"
            | "model"
            | "session"
            | "session_id"
            | "sessionid"
            | "source"
            | "source_kind"
            | "sourcekind"
            | "key"
            | "stable_key"
            | "stablekey"
    )
}

fn ai_vault_pattern_matches_metadata(hit: &AiVaultHit, pattern: &str) -> bool {
    let mut values = hit.metadata_search_terms();
    values.push(hit.stable_key.clone());
    values
        .iter()
        .any(|value| ai_vault_pattern_matches_value(pattern, value))
}

fn ai_vault_pattern_matches_value(pattern: &str, value: &str) -> bool {
    let pattern = normalize_ai_vault_pattern_value(pattern);
    if pattern.is_empty() {
        return false;
    }
    let value = normalize_ai_vault_pattern_value(value);
    if pattern.contains('*') || pattern.contains('?') {
        return wildcard_match(&pattern, &value);
    }
    value.contains(&pattern)
}

fn normalize_ai_vault_pattern_value(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

fn stable_fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let (mut pattern_index, mut value_index) = (0, 0);
    let mut star_index = None;
    let mut star_value_index = 0;

    while value_index < value.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == value[value_index])
        {
            pattern_index += 1;
            value_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_value_index = value_index;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            star_value_index += 1;
            value_index = star_value_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

type AiVaultCache = HashMap<String, (Instant, Vec<AiVaultHit>)>;

fn ai_vault_cache() -> &'static Mutex<AiVaultCache> {
    static CACHE: OnceLock<Mutex<AiVaultCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ai_vault_cache_get(key: &str, ttl_ms: u64) -> Option<Vec<AiVaultHit>> {
    let ttl = Duration::from_millis(ttl_ms);
    let cache = ai_vault_cache().lock().ok()?;
    let (created_at, hits) = cache.get(key)?;
    (created_at.elapsed() <= ttl).then(|| hits.clone())
}

fn ai_vault_cache_put(key: String, hits: Vec<AiVaultHit>) {
    if let Ok(mut cache) = ai_vault_cache().lock() {
        cache.insert(key, (Instant::now(), hits));
        ai_vault_cache_generation().fetch_add(1, Ordering::Relaxed);
    }
}

fn ai_vault_cache_generation() -> &'static AtomicU64 {
    static GENERATION: OnceLock<AtomicU64> = OnceLock::new();
    GENERATION.get_or_init(|| AtomicU64::new(1))
}

fn terminal_routing_value(routing: AiVaultTerminalRouting) -> &'static str {
    match routing {
        AiVaultTerminalRouting::UserPreferred => "userPreferred",
        AiVaultTerminalRouting::NewTerminal => "newTerminal",
    }
}

fn launched_receipt(
    hit: &AiVaultHit,
    terminal_routing: AiVaultTerminalRouting,
    status: &str,
) -> AiVaultResumeReceipt {
    AiVaultResumeReceipt {
        status: status.to_string(),
        provider: hit.provider.clone(),
        session_id: hit.session_id.clone(),
        terminal_routing: terminal_routing_value(terminal_routing).to_string(),
        terminal_target_id: None,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hit() -> AiVaultHit {
        AiVaultHit {
            provider: "codex".to_string(),
            provider_display_name: "Codex".to_string(),
            session_id: "session-123".to_string(),
            source_kind: Some("cli".to_string()),
            safe_title: "Investigate launcher filtering".to_string(),
            workspace_path: Some("/Users/me/dev/script-kit-gpui".to_string()),
            model: Some("gpt-5.6-terra".to_string()),
            modified_at: Some("2026-05-16T00:00:00Z".to_string()),
            matched_field: AiVaultMatchedField::Recent,
            stable_key: "ai-vault/codex/cli/session-123".to_string(),
            score: 0,
            search_terms: Vec::new(),
            search_haystack: String::new(),
            rollout_path: None,
        }
    }

    #[test]
    fn local_query_match_updates_matched_field_without_rescanning() {
        let mut hit = test_hit();
        assert!(apply_local_vault_query_match(&mut hit, "launcher", false));
        assert_eq!(hit.matched_field, AiVaultMatchedField::Title);

        let mut hit = test_hit();
        assert!(apply_local_vault_query_match(
            &mut hit,
            "gpt-5.6-terra",
            false
        ));
        assert_eq!(hit.matched_field, AiVaultMatchedField::Model);

        let mut hit = test_hit();
        assert!(!apply_local_vault_query_match(
            &mut hit,
            "definitely-absent",
            false
        ));
    }

    #[test]
    fn local_index_cache_key_ignores_query_specific_limits() {
        let mut options = RootAiVaultSectionOptions::default();
        options.max_results = 1;
        options.search_content = false;
        let baseline = ai_vault_index_cache_key(&options);

        options.max_results = 5;
        assert_eq!(baseline, ai_vault_index_cache_key(&options));
    }

    #[test]
    fn local_index_cache_key_includes_content_mode() {
        let mut options = RootAiVaultSectionOptions::default();
        options.search_content = false;
        let metadata_only = ai_vault_index_cache_key(&options);
        options.search_content = true;
        assert_ne!(metadata_only, ai_vault_index_cache_key(&options));
    }

    #[test]
    fn codex_state_db_loads_metadata_and_rollout_terms_without_serializing_body() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("state_5.sqlite");
        let rollout_path = temp_dir.path().join("rollout.jsonl");
        std::fs::write(
            &rollout_path,
            r#"{"type":"response_item","payload":{"content":"rollout-only-needle POISON_TRANSCRIPT"}}"#,
        )
        .unwrap();
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT,
                cwd TEXT,
                title TEXT,
                model TEXT,
                git_branch TEXT,
                approval_mode TEXT,
                sandbox_policy TEXT,
                reasoning_effort TEXT,
                first_user_message TEXT,
                updated_at_ms INTEGER,
                archived INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO threads (
                id, rollout_path, cwd, title, model, git_branch, approval_mode,
                sandbox_policy, reasoning_effort, first_user_message, updated_at_ms, archived
            ) VALUES (
                'codex-sql-title-match', '#ROLLOUT#', '/tmp/ai-vault-codex-project',
                'Codex SQL title match', 'gpt-5.1-codex', 'main', 'on-request',
                '{"type":"workspace-write"}', 'high', 'first message fallback',
                1770000000000, 0
            );
            INSERT INTO threads (
                id, rollout_path, cwd, title, model, git_branch, approval_mode,
                sandbox_policy, reasoning_effort, first_user_message, updated_at_ms, archived
            ) VALUES (
                'archived-session', NULL, '/tmp/archived', 'Archived Codex', 'gpt-5.1-codex',
                NULL, NULL, NULL, NULL, NULL, 1770000001000, 1
            );
            "#
            .replace(
                "#ROLLOUT#",
                &rollout_path.to_string_lossy().replace('\'', "''"),
            )
            .as_str(),
        )
        .unwrap();
        drop(conn);

        let mut options = RootAiVaultSectionOptions::default();
        options.search_content = true;
        let hits =
            read_codex_vault_hits_via_state_db(&db_path, temp_dir.path(), &options, 10000).unwrap();
        assert_eq!(hits.len(), 1);
        let hit = hits.first().unwrap();
        assert_eq!(hit.provider, "codex");
        assert_eq!(hit.safe_title, "Codex SQL title match");
        assert_eq!(
            hit.workspace_path.as_deref(),
            Some("/tmp/ai-vault-codex-project")
        );
        assert_eq!(hit.model.as_deref(), Some("gpt-5.1-codex"));

        let mut content_hit = hit.clone();
        assert!(!apply_local_vault_query_match(
            &mut content_hit,
            "rollout-only-needle",
            true
        ));
        hydrate_rollout_search_terms(&mut content_hit);
        assert!(apply_local_vault_query_match(
            &mut content_hit,
            "rollout-only-needle",
            true
        ));
        assert_eq!(content_hit.matched_field, AiVaultMatchedField::Transcript);

        let serialized = serde_json::to_string(hit).unwrap();
        assert!(!serialized.contains("rollout-only-needle"));
        assert!(!serialized.contains("POISON_TRANSCRIPT"));
        assert!(!serialized.contains("rollout_path"));
    }

    #[test]
    fn codex_state_db_missing_falls_back_to_session_index_contract() {
        let temp_dir = tempfile::tempdir().unwrap();
        let missing = temp_dir.path().join("state_5.sqlite");
        let options = RootAiVaultSectionOptions::default();
        let error = read_codex_vault_hits_via_state_db(&missing, temp_dir.path(), &options, 10000)
            .unwrap_err();
        assert!(error.to_string().contains("state_5.sqlite missing"));
    }
}

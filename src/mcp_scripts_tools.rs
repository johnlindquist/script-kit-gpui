use crate::mcp_kit_tools::{ToolContent, ToolDefinition, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const SCRIPTS_CREATE_TOOL: &str = "kit/scripts_create";
pub const SCRIPTS_UPDATE_TOOL: &str = "kit/scripts_update";
pub const SCRIPTS_DELETE_TOOL: &str = "kit/scripts_delete";
pub const SCRIPTS_RUN_TOOL: &str = "kit/scripts_run";
pub const SCRIPT_BODY_MAX_BYTES: usize = 1024 * 1024;
pub const SCRIPT_RUN_DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub const SCRIPT_RUN_MAX_TIMEOUT_MS: u64 = 120_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptsMutationEnvelope {
    pub ok: bool,
    pub action: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ScriptsMutationResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ScriptsMutationError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScriptsMutationErrorCode {
    InvalidParams,
    NotFound,
    Conflict,
    ConfirmRequired,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptsMutationError {
    pub code: ScriptsMutationErrorCode,
    pub message: String,
}

impl ScriptsMutationError {
    pub fn new(code: ScriptsMutationErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(ScriptsMutationErrorCode::InvalidParams, message)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptsCreateArgs {
    pub name: String,
    #[serde(alias = "content")]
    pub body: String,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptsUpdateArgs {
    pub name: String,
    #[serde(alias = "content")]
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptsDeleteArgs {
    pub name: String,
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptsRunArgs {
    pub name: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub stdin: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum ScriptsMutationRequest {
    Create(ScriptsCreateArgs),
    Update(ScriptsUpdateArgs),
    Delete(ScriptsDeleteArgs),
    Run(ScriptsRunArgs),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptsMutationResult {
    pub name: String,
    pub path: String,
    pub uri: String,
    pub created: bool,
    pub updated: bool,
    pub deleted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timed_out: Option<bool>,
}

pub fn is_scripts_tool(name: &str) -> bool {
    matches!(
        name,
        SCRIPTS_CREATE_TOOL | SCRIPTS_UPDATE_TOOL | SCRIPTS_DELETE_TOOL | SCRIPTS_RUN_TOOL
    )
}

pub fn get_scripts_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: SCRIPTS_CREATE_TOOL.to_string(),
            description: "Create a TypeScript script under ~/.scriptkit/plugins/main/scripts."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "name": { "type": "string", "description": "Script filename stem. Sanitized like Script Kit's New Script flow." },
                    "body": { "type": "string", "maxLength": SCRIPT_BODY_MAX_BYTES },
                    "content": { "type": "string", "maxLength": SCRIPT_BODY_MAX_BYTES },
                    "overwrite": { "type": "boolean", "default": false }
                },
                "required": ["name", "body"]
            }),
        },
        ToolDefinition {
            name: SCRIPTS_UPDATE_TOOL.to_string(),
            description: "Replace an existing TypeScript script under ~/.scriptkit/plugins/main/scripts."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "name": { "type": "string" },
                    "body": { "type": "string", "maxLength": SCRIPT_BODY_MAX_BYTES },
                    "content": { "type": "string", "maxLength": SCRIPT_BODY_MAX_BYTES }
                },
                "required": ["name", "body"]
            }),
        },
        ToolDefinition {
            name: SCRIPTS_DELETE_TOOL.to_string(),
            description: "Delete a TypeScript script from ~/.scriptkit/plugins/main/scripts. Requires confirm:true."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "name": { "type": "string" },
                    "confirm": { "type": "boolean", "default": false }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: SCRIPTS_RUN_TOOL.to_string(),
            description: "Run a TypeScript script from ~/.scriptkit/plugins/main/scripts with Bun and return stdout/stderr/exit status."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "name": { "type": "string" },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "default": []
                    },
                    "env": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "default": {}
                    },
                    "stdin": { "type": "string" },
                    "timeoutMs": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": SCRIPT_RUN_MAX_TIMEOUT_MS,
                        "default": SCRIPT_RUN_DEFAULT_TIMEOUT_MS
                    }
                },
                "required": ["name"]
            }),
        },
    ]
}

pub fn handle_scripts_tool_call(name: &str, arguments: Value) -> ToolResult {
    let request = match parse_scripts_mutation_request(name, arguments) {
        Ok(request) => request,
        Err(error) => return error_tool_result(name, error),
    };

    match apply_scripts_mutation(request) {
        Ok((action, result)) => success_tool_result(action, result),
        Err((action, error)) => error_tool_result(action, error),
    }
}

pub fn parse_scripts_mutation_request(
    name: &str,
    arguments: Value,
) -> Result<ScriptsMutationRequest, ScriptsMutationError> {
    match name {
        SCRIPTS_CREATE_TOOL => {
            let args: ScriptsCreateArgs = serde_json::from_value(arguments).map_err(|error| {
                ScriptsMutationError::invalid_params(format!(
                    "Invalid scripts_create arguments: {error}"
                ))
            })?;
            validate_body_len(&args.body)?;
            Ok(ScriptsMutationRequest::Create(args))
        }
        SCRIPTS_UPDATE_TOOL => {
            let args: ScriptsUpdateArgs = serde_json::from_value(arguments).map_err(|error| {
                ScriptsMutationError::invalid_params(format!(
                    "Invalid scripts_update arguments: {error}"
                ))
            })?;
            validate_body_len(&args.body)?;
            Ok(ScriptsMutationRequest::Update(args))
        }
        SCRIPTS_DELETE_TOOL => {
            let args: ScriptsDeleteArgs = serde_json::from_value(arguments).map_err(|error| {
                ScriptsMutationError::invalid_params(format!(
                    "Invalid scripts_delete arguments: {error}"
                ))
            })?;
            if !args.confirm {
                return Err(ScriptsMutationError::new(
                    ScriptsMutationErrorCode::ConfirmRequired,
                    "kit/scripts_delete requires confirm:true",
                ));
            }
            Ok(ScriptsMutationRequest::Delete(args))
        }
        SCRIPTS_RUN_TOOL => {
            let args: ScriptsRunArgs = serde_json::from_value(arguments).map_err(|error| {
                ScriptsMutationError::invalid_params(format!(
                    "Invalid scripts_run arguments: {error}"
                ))
            })?;
            if let Some(timeout_ms) = args.timeout_ms {
                if timeout_ms == 0 || timeout_ms > SCRIPT_RUN_MAX_TIMEOUT_MS {
                    return Err(ScriptsMutationError::invalid_params(format!(
                        "timeoutMs must be between 1 and {SCRIPT_RUN_MAX_TIMEOUT_MS}"
                    )));
                }
            }
            Ok(ScriptsMutationRequest::Run(args))
        }
        _ => Err(ScriptsMutationError::invalid_params(format!(
            "Unknown scripts tool: {name}"
        ))),
    }
}

fn apply_scripts_mutation(
    request: ScriptsMutationRequest,
) -> Result<(&'static str, ScriptsMutationResult), (&'static str, ScriptsMutationError)> {
    match request {
        ScriptsMutationRequest::Create(args) => create_script(args),
        ScriptsMutationRequest::Update(args) => update_script(args),
        ScriptsMutationRequest::Delete(args) => delete_script(args),
        ScriptsMutationRequest::Run(args) => run_script(args),
    }
}

fn create_script(
    args: ScriptsCreateArgs,
) -> Result<(&'static str, ScriptsMutationResult), (&'static str, ScriptsMutationError)> {
    let action = "scripts_create";
    let path = script_path_for_name(&args.name).map_err(|error| (action, error))?;
    if path.exists() && !args.overwrite {
        return Err((
            action,
            ScriptsMutationError::new(
                ScriptsMutationErrorCode::Conflict,
                format!("Script already exists: {}", path.display()),
            ),
        ));
    }
    write_script_file(&path, &args.body).map_err(|error| (action, error))?;
    Ok((
        action,
        result_for_path(&path, !args.overwrite, args.overwrite, false),
    ))
}

fn update_script(
    args: ScriptsUpdateArgs,
) -> Result<(&'static str, ScriptsMutationResult), (&'static str, ScriptsMutationError)> {
    let action = "scripts_update";
    let path = script_path_for_name(&args.name).map_err(|error| (action, error))?;
    if !path.exists() {
        return Err((
            action,
            ScriptsMutationError::new(
                ScriptsMutationErrorCode::NotFound,
                format!("Script not found: {}", path.display()),
            ),
        ));
    }
    write_script_file(&path, &args.body).map_err(|error| (action, error))?;
    Ok((action, result_for_path(&path, false, true, false)))
}

fn delete_script(
    args: ScriptsDeleteArgs,
) -> Result<(&'static str, ScriptsMutationResult), (&'static str, ScriptsMutationError)> {
    let action = "scripts_delete";
    let path = script_path_for_name(&args.name).map_err(|error| (action, error))?;
    if !path.exists() {
        return Err((
            action,
            ScriptsMutationError::new(
                ScriptsMutationErrorCode::NotFound,
                format!("Script not found: {}", path.display()),
            ),
        ));
    }
    std::fs::remove_file(&path).map_err(|error| {
        (
            action,
            ScriptsMutationError::new(
                ScriptsMutationErrorCode::Internal,
                format!("Failed to delete script {}: {error}", path.display()),
            ),
        )
    })?;
    Ok((action, result_for_path(&path, false, false, true)))
}

fn run_script(
    args: ScriptsRunArgs,
) -> Result<(&'static str, ScriptsMutationResult), (&'static str, ScriptsMutationError)> {
    let action = "scripts_run";
    let path = script_path_for_name(&args.name).map_err(|error| (action, error))?;
    if !path.exists() {
        return Err((
            action,
            ScriptsMutationError::new(
                ScriptsMutationErrorCode::NotFound,
                format!("Script not found: {}", path.display()),
            ),
        ));
    }

    let output = run_bun_script(&path, &args).map_err(|error| (action, error))?;
    let mut result = result_for_path(&path, false, false, false);
    result.exit_code = output.exit_code;
    result.stdout = Some(output.stdout);
    result.stderr = Some(output.stderr);
    result.timed_out = Some(output.timed_out);
    Ok((action, result))
}

struct ScriptRunOutput {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

fn run_bun_script(
    path: &Path,
    args: &ScriptsRunArgs,
) -> Result<ScriptRunOutput, ScriptsMutationError> {
    let timeout_ms = args.timeout_ms.unwrap_or(SCRIPT_RUN_DEFAULT_TIMEOUT_MS);
    let mut command = Command::new("bun");
    command
        .arg(path)
        .args(&args.args)
        .envs(&args.env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if args.stdin.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }

    let mut child = command.spawn().map_err(|error| {
        ScriptsMutationError::new(
            ScriptsMutationErrorCode::Internal,
            format!("Failed to start bun for script {}: {error}", path.display()),
        )
    })?;

    if let Some(stdin) = &args.stdin {
        if let Some(mut child_stdin) = child.stdin.take() {
            use std::io::Write;
            child_stdin.write_all(stdin.as_bytes()).map_err(|error| {
                ScriptsMutationError::new(
                    ScriptsMutationErrorCode::Internal,
                    format!(
                        "Failed to write stdin for script {}: {error}",
                        path.display()
                    ),
                )
            })?;
        }
    }

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        match child.try_wait().map_err(|error| {
            ScriptsMutationError::new(
                ScriptsMutationErrorCode::Internal,
                format!("Failed to poll script {}: {error}", path.display()),
            )
        })? {
            Some(_) => {
                let output = child.wait_with_output().map_err(|error| {
                    ScriptsMutationError::new(
                        ScriptsMutationErrorCode::Internal,
                        format!(
                            "Failed to collect script output {}: {error}",
                            path.display()
                        ),
                    )
                })?;
                return Ok(ScriptRunOutput {
                    exit_code: output.status.code(),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    timed_out: false,
                });
            }
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let output = child.wait_with_output().map_err(|error| {
                    ScriptsMutationError::new(
                        ScriptsMutationErrorCode::Internal,
                        format!(
                            "Failed to collect timed-out script output {}: {error}",
                            path.display()
                        ),
                    )
                })?;
                return Ok(ScriptRunOutput {
                    exit_code: output.status.code(),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    timed_out: true,
                });
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn script_path_for_name(name: &str) -> Result<PathBuf, ScriptsMutationError> {
    let stem = crate::script_creation::sanitize_name(name);
    if stem.is_empty() {
        return Err(ScriptsMutationError::invalid_params(format!(
            "Script name cannot be empty after sanitization: {name}"
        )));
    }
    let path = crate::script_creation::scripts_dir().join(format!("{stem}.ts"));
    ensure_within_scripts_dir(&path)?;
    Ok(path)
}

fn ensure_within_scripts_dir(path: &Path) -> Result<(), ScriptsMutationError> {
    let scripts_dir = crate::script_creation::scripts_dir();
    if path.parent() != Some(scripts_dir.as_path()) {
        return Err(ScriptsMutationError::invalid_params(format!(
            "Script path must stay inside {}",
            scripts_dir.display()
        )));
    }
    Ok(())
}

fn write_script_file(path: &Path, body: &str) -> Result<(), ScriptsMutationError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            ScriptsMutationError::new(
                ScriptsMutationErrorCode::Internal,
                format!(
                    "Failed to create scripts directory {}: {error}",
                    parent.display()
                ),
            )
        })?;
    }
    std::fs::write(path, body).map_err(|error| {
        ScriptsMutationError::new(
            ScriptsMutationErrorCode::Internal,
            format!("Failed to write script {}: {error}", path.display()),
        )
    })
}

fn result_for_path(
    path: &Path,
    created: bool,
    updated: bool,
    deleted: bool,
) -> ScriptsMutationResult {
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string();
    ScriptsMutationResult {
        uri: format!("scripts://{name}"),
        name,
        path: path.to_string_lossy().to_string(),
        created,
        updated,
        deleted,
        exit_code: None,
        stdout: None,
        stderr: None,
        timed_out: None,
    }
}

fn validate_body_len(body: &str) -> Result<(), ScriptsMutationError> {
    if body.len() > SCRIPT_BODY_MAX_BYTES {
        return Err(ScriptsMutationError::invalid_params(format!(
            "Script body exceeds {SCRIPT_BODY_MAX_BYTES} byte limit"
        )));
    }
    Ok(())
}

fn success_tool_result(action: &'static str, result: ScriptsMutationResult) -> ToolResult {
    let resource_uri = Some(result.uri.clone());
    envelope_tool_result(ScriptsMutationEnvelope {
        ok: true,
        action,
        resource_uri,
        result: Some(result),
        error: None,
    })
}

pub fn error_tool_result(action: &str, error: ScriptsMutationError) -> ToolResult {
    let mut result = envelope_tool_result(ScriptsMutationEnvelope {
        ok: false,
        action: action_label(action),
        resource_uri: None,
        result: None,
        error: Some(error),
    });
    result.is_error = Some(true);
    result
}

fn action_label(action: &str) -> &'static str {
    match action {
        SCRIPTS_CREATE_TOOL => "scripts_create",
        SCRIPTS_UPDATE_TOOL => "scripts_update",
        SCRIPTS_DELETE_TOOL => "scripts_delete",
        SCRIPTS_RUN_TOOL => "scripts_run",
        "scripts_create" => "scripts_create",
        "scripts_update" => "scripts_update",
        "scripts_delete" => "scripts_delete",
        "scripts_run" => "scripts_run",
        _ => "scripts_unknown",
    }
}

fn envelope_tool_result(envelope: ScriptsMutationEnvelope) -> ToolResult {
    ToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: serde_json::to_string(&envelope).unwrap_or_else(|error| {
                format!(
                    r#"{{"ok":false,"action":"scripts_internal","error":{{"code":"internal","message":"Failed to serialize scripts result: {error}"}}}}"#
                )
            }),
        }],
        is_error: None,
    }
}

#[cfg(test)]
mod mcp_scripts_fuzz_tests {
    use super::*;
    use serde_json::json;

    /// The scripts MCP handler's parse layer must never panic on hostile input,
    /// and the path guard must never let a name escape the scripts directory.
    /// (Battery A — empirical fuzz of the prime MCP input boundary; parse/path
    /// only, never the spawning/writing apply_* layer.)
    #[test]
    fn parse_scripts_requests_never_panic_on_hostile_args() {
        let tools = [
            SCRIPTS_CREATE_TOOL,
            SCRIPTS_UPDATE_TOOL,
            SCRIPTS_DELETE_TOOL,
            SCRIPTS_RUN_TOOL,
        ];
        let hostile_args = vec![
            json!(null),
            json!(42),
            json!("a string, not an object"),
            json!([]),
            json!({}),              // missing required
            json!({ "name": 123 }), // wrong type
            json!({ "name": null, "body": null }),
            json!({ "name": "x", "body": 999 }), // body wrong type
            json!({ "name": "x", "body": "y", "unexpected": true }), // deny_unknown_fields
            json!({ "name": "x", "body": "x".repeat(2_000_000) }), // oversized body
            json!({ "name": "x", "confirm": "yes" }), // confirm wrong type
            json!({ "name": "x", "timeoutMs": -5 }),
            json!({ "name": "x", "timeoutMs": 999999999999i64 }),
            json!({ "name": "x", "args": "not-an-array" }),
            json!({ "name": "x", "env": [1, 2, 3] }),
            json!({ "name": "🎉\u{0000}", "body": "z" }),
            json!({ "name": { "nested": "object" }, "body": "z" }),
        ];
        for tool in tools {
            for args in &hostile_args {
                // Must return Ok/Err, never panic.
                let _ = parse_scripts_mutation_request(tool, args.clone());
            }
        }
    }

    #[test]
    fn script_path_for_name_never_escapes_scripts_dir() {
        let scripts_dir = crate::script_creation::scripts_dir();
        let hostile_names = [
            "",
            ".",
            "..",
            "../..",
            "../../etc/passwd",
            "/etc/passwd",
            "/../../../../etc/shadow",
            "foo/bar",
            "foo\\bar",
            "a/../../b",
            "....//....//etc",
            "a\u{0000}b",
            "..\u{2044}..\u{2044}etc", // unicode fraction slash
            "\u{202e}gpj.exe",         // RTL override
            "con",
            "PRN",
            &"a".repeat(50_000),
            "  ../../  ",
            "🎉../../🎉",
            "文件/../../secret",
        ];
        for name in hostile_names {
            match script_path_for_name(name) {
                Ok(path) => {
                    // If a path is produced, it MUST live directly inside the
                    // scripts dir — never an ancestor / sibling / absolute escape.
                    assert_eq!(
                        path.parent(),
                        Some(scripts_dir.as_path()),
                        "name {name:?} produced an out-of-tree path: {}",
                        path.display()
                    );
                    // And the file name must be the sanitized stem + .ts (no separators).
                    let fname = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
                    assert!(
                        !fname.contains('/') && !fname.contains('\\') && !fname.contains(".."),
                        "name {name:?} produced a traversal filename: {fname:?}"
                    );
                }
                Err(_) => { /* rejected (e.g. empty after sanitize) — also fine */ }
            }
        }
    }
}

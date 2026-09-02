//! Scriptlet Execution
//!
//! This module handles execution of scriptlets (small scripts embedded in markdown)
//! with support for various tool types (shell, scripting languages, TypeScript, etc.)

use crate::scriptlets::{
    format_scriptlet, normalize_scriptlet_tool, process_conditionals, Scriptlet, SHELL_TOOLS,
};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;
use tracing::{debug, error, info, instrument, warn};

use super::runner::find_executable;
use super::runner::find_sdk_path;

// Conditionally import selected_text for macOS only
#[cfg(target_os = "macos")]
use crate::selected_text;

const SAFE_SCRIPTLET_ENV_VARS: [&str; 9] = [
    "PATH", "HOME", "TMPDIR", "TEMP", "TMP", "USER", "LANG", "TERM", "SHELL",
];

// Scriptlets are untrusted content. Clear inherited environment variables to avoid leaking
// parent-process secrets (tokens, credentials) and re-add only minimal execution basics.
fn apply_scriptlet_environment_allowlist(cmd: &mut Command) {
    cmd.env_clear();

    for env_key in SAFE_SCRIPTLET_ENV_VARS {
        if let Some(env_value) = std::env::var_os(env_key) {
            cmd.env(env_key, env_value);
        }
    }
}

fn apply_scriptlet_environment(cmd: &mut Command, options: &ScriptletExecOptions) {
    apply_scriptlet_environment_allowlist(cmd);
    for (key, value) in &options.extra_env {
        cmd.env(key, value);
    }
}

fn format_template_content(
    content: &str,
    inputs: &HashMap<String, String>,
    positional_args: &[String],
    windows: bool,
) -> String {
    let mut result = content.to_string();

    for (name, value) in inputs {
        let placeholder = format!("{{{{{}}}}}", name);
        result = result.replace(&placeholder, value);
    }

    if windows {
        for (index, arg) in positional_args.iter().enumerate() {
            let placeholder = format!("%{}", index + 1);
            result = result.replace(&placeholder, arg);
        }
        result = result.replace("%*", &positional_args.join(" "));
    } else {
        for (index, arg) in positional_args.iter().enumerate() {
            let placeholder = format!("${}", index + 1);
            result = result.replace(&placeholder, arg);
        }
        result = result.replace("$@", &positional_args.join(" "));
    }

    result
}

/// Options for scriptlet execution
#[derive(Debug, Clone, Default)]
pub struct ScriptletExecOptions {
    /// Current working directory for script execution
    pub cwd: Option<PathBuf>,
    /// Commands to prepend before the main script content
    pub prepend: Option<String>,
    /// Commands to append after the main script content
    pub append: Option<String>,
    /// Named inputs for variable substitution
    pub inputs: HashMap<String, String>,
    /// Positional arguments for variable substitution
    pub positional_args: Vec<String>,
    /// Extra environment pairs to expose after the safe allowlist is restored.
    pub extra_env: Vec<(String, String)>,
    /// Flags for conditional processing
    pub flags: HashMap<String, bool>,
}

/// Result of a scriptlet execution
#[derive(Debug)]
pub struct ScriptletResult {
    /// Exit code (0 = success)
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Whether execution was successful
    pub success: bool,
}

/// Get the file extension for a given tool type
#[cfg(test)]
pub fn tool_extension(tool: &str) -> &'static str {
    match tool {
        "ruby" => "rb",
        "python" => "py",
        "perl" => "pl",
        "php" => "php",
        "bash" | "sh" => "sh",
        "zsh" => "zsh",
        "fish" => "fish",
        "node" | "js" => "js",
        "ts" | "kit" | "bun" | "deno" => "ts",
        "applescript" => "applescript",
        "powershell" | "pwsh" => "ps1",
        "cmd" => "bat",
        _ => "sh", // Default to shell script
    }
}

/// Execute a scriptlet based on its tool type
///
/// # Arguments
/// * `scriptlet` - The scriptlet to execute
/// * `options` - Execution options (cwd, prepend, append, inputs, etc.)
///
/// # Returns
/// A `ScriptletResult` with exit code, stdout, stderr, and success flag
///
/// # Tool Types Supported
/// - Shell (bash, zsh, sh, fish): Write temp file, execute via shell
/// - Scripting (python, ruby, perl, php, node): Write temp file with extension, execute
/// - TypeScript (kit, ts, bun, deno): Write temp .ts file, run via bun
/// - transform: Wrap with getSelectedText/setSelectedText (macOS only)
/// - template: Returns content for template prompt invocation
/// - open: Use `open` command (macOS) or `xdg-open` (Linux)
/// - edit: Open in editor
/// - paste: Set selected text via clipboard
/// - type: Simulate keyboard typing
/// - submit: Paste + enter
#[instrument(skip_all, fields(tool = %scriptlet.tool, name = %scriptlet.name))]
pub fn run_scriptlet(
    scriptlet: &Scriptlet,
    options: ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)
        .map_err(|error| error.to_string())?;
    let capability_issues = crate::scripts::validate_legacy_scriptlet_capabilities(scriptlet);
    if let Some(issue) = capability_issues
        .iter()
        .find(|issue| issue.severity == crate::scripts::ValidationSeverity::Fatal)
        .or_else(|| capability_issues.first())
    {
        warn!(
            target: "script_kit::execution",
            scriptlet = %scriptlet.name,
            issue = ?issue.kind,
            "legacy_scriptlet_capability_preflight_refused_before_execution"
        );
        return Err(format!(
            "Scriptlet `{}` cannot run: {}",
            scriptlet.name, issue.message
        ));
    }

    let start = Instant::now();
    debug!(tool = %scriptlet.tool, name = %scriptlet.name, "Running scriptlet");
    info!(
        category = "EXEC",
        scriptlet_name = %scriptlet.name,
        tool = %scriptlet.tool,
        "Running scriptlet"
    );

    // Process conditionals and variable substitution
    let content = process_conditionals(&scriptlet.scriptlet_content, &options.flags);
    let is_windows = cfg!(target_os = "windows");
    let tool = normalize_scriptlet_tool(&scriptlet.tool);
    let content = if tool == "template" {
        format_template_content(
            &content,
            &options.inputs,
            &options.positional_args,
            is_windows,
        )
    } else {
        format_scriptlet(
            &content,
            &options.inputs,
            &options.positional_args,
            is_windows,
        )
    };

    // Apply prepend/append
    let content = build_final_content(&content, &options.prepend, &options.append);

    let result = match tool.as_str() {
        // Shell tools
        t if SHELL_TOOLS.contains(&t) => execute_shell_scriptlet(&tool, &content, &options),

        // Scripting languages
        "python" => execute_with_interpreter("python3", &content, "py", &options),
        "ruby" => execute_with_interpreter("ruby", &content, "rb", &options),
        "perl" => execute_with_interpreter("perl", &content, "pl", &options),
        "php" => execute_with_interpreter("php", &content, "php", &options),
        "node" | "js" => execute_with_interpreter("node", &content, "js", &options),
        "applescript" => execute_applescript(&content, &options),

        // TypeScript tools (run via bun)
        "kit" | "ts" | "bun" | "deno" => execute_typescript(&content, &options),

        // Transform (get selected text, process, set selected text)
        "transform" => execute_transform(&content, &options),

        // Template (return content for prompt invocation)
        "template" => {
            // Template just returns the processed content - the caller handles prompt invocation
            Ok(ScriptletResult {
                exit_code: 0,
                stdout: content,
                stderr: String::new(),
                success: true,
            })
        }

        // Open URL/file
        "open" => execute_open(&content, &options),

        // Edit file in editor
        "edit" => execute_edit(&content, &options),

        // Paste text (set selected text)
        "paste" => execute_paste(&content),

        // Type text via keyboard simulation
        "type" => execute_type(&content),

        // Submit (paste + enter)
        "submit" => execute_submit(&content),

        // Unknown tool - try as shell
        _ => {
            warn!(tool = %tool, "Unknown tool type, falling back to shell");
            execute_shell_scriptlet("sh", &content, &options)
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;
    match &result {
        Ok(r) => {
            info!(
                duration_ms = duration_ms,
                exit_code = r.exit_code,
                tool = %tool,
                "Scriptlet execution complete"
            );
            info!(
                category = "EXEC",
                scriptlet_name = %scriptlet.name,
                exit_code = r.exit_code,
                duration_ms,
                "Scriptlet completed"
            );
        }
        Err(e) => {
            error!(duration_ms = duration_ms, error = %e, tool = %tool, "Scriptlet execution failed");
            info!(
                category = "EXEC",
                scriptlet_name = %scriptlet.name,
                error = %e,
                "Scriptlet failed"
            );
        }
    }

    result
}

/// Build final content with prepend/append
pub fn build_final_content(
    content: &str,
    prepend: &Option<String>,
    append: &Option<String>,
) -> String {
    let mut result = String::new();

    if let Some(pre) = prepend {
        result.push_str(pre);
        if !pre.ends_with('\n') {
            result.push('\n');
        }
    }

    result.push_str(content);

    if let Some(app) = append {
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(app);
    }

    result
}

#[derive(Clone, Copy, Debug)]
enum TempScriptMode {
    Executable,
    InterpreterFed,
}

#[cfg(unix)]
fn temp_script_unix_mode(mode: TempScriptMode) -> u32 {
    match mode {
        TempScriptMode::Executable => 0o700,
        TempScriptMode::InterpreterFed => 0o600,
    }
}

#[cfg(unix)]
fn apply_secure_temp_permissions(file: &std::fs::File, mode: TempScriptMode) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let unix_mode = temp_script_unix_mode(mode);
    let mut permissions = file
        .metadata()
        .map_err(|error| {
            format!(
                "secure_tempfile_metadata_failed: attempted=read_metadata mode={:o} error={}",
                unix_mode, error
            )
        })?
        .permissions();
    permissions.set_mode(unix_mode);
    file.set_permissions(permissions).map_err(|error| {
        format!(
            "secure_tempfile_permissions_failed: attempted=set_permissions mode={:o} error={}",
            unix_mode, error
        )
    })
}

fn create_secure_temp_script(
    content: &str,
    suffix: &str,
    mode: TempScriptMode,
) -> Result<NamedTempFile, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)
        .map_err(|error| error.to_string())?;
    debug!(
        suffix = %suffix,
        temp_mode = ?mode,
        "Creating secure temp script file"
    );

    let mut temp_file = tempfile::Builder::new()
        .prefix("scriptlet-")
        .suffix(suffix)
        .tempfile()
        .map_err(|error| {
            format!(
                "secure_tempfile_create_failed: attempted=create_tempfile suffix={} error={}",
                suffix, error
            )
        })?;

    temp_file
        .as_file_mut()
        .write_all(content.as_bytes())
        .map_err(|error| {
            format!(
                "secure_tempfile_write_failed: attempted=write_content suffix={} error={}",
                suffix, error
            )
        })?;
    temp_file.as_file_mut().flush().map_err(|error| {
        format!(
            "secure_tempfile_flush_failed: attempted=flush_content suffix={} error={}",
            suffix, error
        )
    })?;

    #[cfg(unix)]
    apply_secure_temp_permissions(temp_file.as_file(), mode)?;

    Ok(temp_file)
}

#[cfg(all(test, unix))]
mod secure_tempfile_tests {
    use super::{create_secure_temp_script, TempScriptMode};
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn test_create_secure_temp_script_sets_mode_700_when_executable() {
        let temp_file =
            create_secure_temp_script("echo secure-tempfiles", ".sh", TempScriptMode::Executable)
                .expect("executable temp script should be created");

        let mode = temp_file
            .as_file()
            .metadata()
            .expect("metadata should be readable")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700, "executable script mode should be 0o700");
    }

    #[test]
    fn test_create_secure_temp_script_sets_mode_600_when_interpreter_fed() {
        let temp_file = create_secure_temp_script(
            "print('secure-tempfiles')",
            ".py",
            TempScriptMode::InterpreterFed,
        )
        .expect("interpreter temp script should be created");

        let mode = temp_file
            .as_file()
            .metadata()
            .expect("metadata should be readable")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "interpreter script mode should be 0o600");
    }

    #[test]
    fn test_create_secure_temp_script_generates_unique_paths_on_consecutive_calls() {
        let first = create_secure_temp_script("echo first", ".sh", TempScriptMode::Executable)
            .expect("first temp script should be created");
        let second = create_secure_temp_script("echo second", ".sh", TempScriptMode::Executable)
            .expect("second temp script should be created");

        assert_ne!(
            first.path(),
            second.path(),
            "secure temp script paths should be random and unique"
        );
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod scriptlet_environment_allowlist_tests {
    use super::{
        apply_scriptlet_environment, apply_scriptlet_environment_allowlist, ScriptletExecOptions,
        SAFE_SCRIPTLET_ENV_VARS,
    };
    use std::process::Command;

    #[test]
    fn test_apply_scriptlet_environment_allowlist_only_includes_safe_keys() {
        let mut cmd = Command::new("sh");
        cmd.env("SCRIPTLET_ENV_SHOULD_NOT_LEAK", "secret");
        apply_scriptlet_environment_allowlist(&mut cmd);

        let allowlist = SAFE_SCRIPTLET_ENV_VARS;
        let contains_disallowed = cmd.get_envs().any(|(key, value)| {
            value.is_some()
                && !allowlist
                    .iter()
                    .any(|allowed_key| key.eq_ignore_ascii_case(allowed_key))
        });
        let contains_leaked_value = cmd.get_envs().any(|(key, value)| {
            value.is_some() && key.eq_ignore_ascii_case("SCRIPTLET_ENV_SHOULD_NOT_LEAK")
        });

        assert!(
            !contains_disallowed,
            "command environment should only contain allowlisted keys"
        );
        assert!(
            !contains_leaked_value,
            "non-allowlisted variables should be removed by env_clear()"
        );
    }

    #[test]
    fn test_apply_scriptlet_environment_allowlist_keeps_path_when_available() {
        if std::env::var_os("PATH").is_none() {
            return;
        }

        let mut cmd = Command::new("sh");
        apply_scriptlet_environment_allowlist(&mut cmd);

        let has_path = cmd
            .get_envs()
            .any(|(key, value)| value.is_some() && key.eq_ignore_ascii_case("PATH"));
        assert!(has_path, "PATH should remain available when present");
    }

    #[test]
    fn test_apply_scriptlet_environment_adds_explicit_extra_env() {
        let mut cmd = Command::new("sh");
        let options = ScriptletExecOptions {
            extra_env: vec![(
                "KIT_MENU_SYNTAX_COMMAND_FIELDS".to_string(),
                r#"[["env","prod"]]"#.to_string(),
            )],
            ..Default::default()
        };
        apply_scriptlet_environment(&mut cmd, &options);

        let has_extra = cmd.get_envs().any(|(key, value)| {
            key.eq_ignore_ascii_case("KIT_MENU_SYNTAX_COMMAND_FIELDS")
                && value.map(|v| v == r#"[["env","prod"]]"#).unwrap_or(false)
        });
        assert!(has_extra, "explicit command env should be re-added");
    }
}

/// Execute a shell scriptlet (bash, zsh, sh, fish, etc.)
#[tracing::instrument(skip(content, options), fields(shell = %shell, content_len = content.len()))]
pub fn execute_shell_scriptlet(
    shell: &str,
    content: &str,
    options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)
        .map_err(|error| error.to_string())?;
    info!(
        category = "EXEC",
        shell = %shell,
        "Executing shell scriptlet"
    );

    let temp_file = create_secure_temp_script(content, ".sh", TempScriptMode::Executable)?;

    // Find the shell executable
    let shell_path = find_executable(shell)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| shell.to_string());

    let mut cmd = Command::new(&shell_path);
    cmd.arg(temp_file.path());

    if let Some(ref cwd) = options.cwd {
        cmd.current_dir(cwd);
    }
    apply_scriptlet_environment(&mut cmd, options);

    let output = cmd.output().map_err(|e| {
        // Provide helpful error message with installation suggestions
        let suggestions = shell_not_found_suggestions(shell);
        format!(
            "Failed to execute shell script with '{}': {}\n\n{}",
            shell, e, suggestions
        )
    })?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Get installation suggestions for a missing shell
pub fn shell_not_found_suggestions(shell: &str) -> String {
    let install_hint = match shell {
        "bash" => {
            if cfg!(target_os = "macos") {
                "bash is usually pre-installed on macOS. Try: brew install bash"
            } else if cfg!(target_os = "linux") {
                "Install with: apt install bash (Debian/Ubuntu) or yum install bash (RHEL/CentOS)"
            } else {
                "bash is typically available through Git for Windows or WSL"
            }
        }
        "zsh" => {
            if cfg!(target_os = "macos") {
                "zsh is the default shell on macOS. If missing, try: brew install zsh"
            } else if cfg!(target_os = "linux") {
                "Install with: apt install zsh (Debian/Ubuntu) or yum install zsh (RHEL/CentOS)"
            } else {
                "zsh can be installed through WSL or Git Bash on Windows"
            }
        }
        "sh" => "sh (POSIX shell) should be available on all Unix systems. Check your PATH.",
        "fish" => {
            if cfg!(target_os = "macos") {
                "Install with: brew install fish"
            } else if cfg!(target_os = "linux") {
                "Install with: apt install fish (Debian/Ubuntu) or check https://fishshell.com"
            } else {
                "fish can be installed through WSL on Windows. See https://fishshell.com"
            }
        }
        "cmd" => {
            if cfg!(target_os = "windows") {
                "cmd.exe should be available at C:\\Windows\\System32\\cmd.exe"
            } else {
                "cmd is a Windows-only shell. On Unix, use bash, zsh, or sh instead."
            }
        }
        "powershell" => {
            if cfg!(target_os = "windows") {
                "PowerShell should be pre-installed on Windows. Check System32\\WindowsPowerShell"
            } else {
                "For cross-platform PowerShell, install pwsh: https://aka.ms/install-powershell"
            }
        }
        "pwsh" => {
            "Install PowerShell Core from: https://aka.ms/install-powershell\n\
             macOS: brew install powershell\n\
             Linux: See https://docs.microsoft.com/powershell/scripting/install/installing-powershell-on-linux"
        }
        _ => "Shell not recognized. Make sure it is installed and in your PATH.",
    };

    format!(
        "Suggestions:\n\
         - Make sure '{}' is installed and accessible in your PATH\n\
         - {}\n\
         - Alternative shells in SHELL_TOOLS: bash, zsh, sh, fish, cmd, powershell, pwsh",
        shell, install_hint
    )
}

/// Execute a script with a specific interpreter
#[tracing::instrument(skip(content, options), fields(interpreter = %interpreter, extension = %extension, content_len = content.len()))]
pub fn execute_with_interpreter(
    interpreter: &str,
    content: &str,
    extension: &str,
    options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)
        .map_err(|error| error.to_string())?;
    info!(
        category = "EXEC",
        interpreter = %interpreter,
        "Executing script with interpreter"
    );

    // Create temp file with appropriate extension
    let suffix = format!(".{}", extension);
    let temp_file = create_secure_temp_script(content, &suffix, TempScriptMode::InterpreterFed)?;

    // Find the interpreter
    let interp_path = find_executable(interpreter)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| interpreter.to_string());

    let mut cmd = Command::new(&interp_path);
    cmd.arg(temp_file.path());

    if let Some(ref cwd) = options.cwd {
        cmd.current_dir(cwd);
    }
    apply_scriptlet_environment(&mut cmd, options);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute {} script: {}", interpreter, e))?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Execute AppleScript
pub fn execute_applescript(
    content: &str,
    options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)
        .map_err(|error| error.to_string())?;
    info!(category = "EXEC", "Executing AppleScript");

    let mut cmd = Command::new("osascript");
    cmd.arg("-e").arg(content);

    if let Some(ref cwd) = options.cwd {
        cmd.current_dir(cwd);
    }
    apply_scriptlet_environment(&mut cmd, options);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Execute TypeScript via bun
#[tracing::instrument(skip(content, options), fields(content_len = content.len()))]
pub fn execute_typescript(
    content: &str,
    options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)
        .map_err(|error| error.to_string())?;
    info!(category = "EXEC", "Executing TypeScript via bun");

    let temp_file = create_secure_temp_script(content, ".ts", TempScriptMode::InterpreterFed)?;

    // Find bun
    let bun_path = find_executable("bun")
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bun".to_string());

    // Check if we should use SDK preload
    let sdk_path = find_sdk_path();

    let mut cmd = Command::new(&bun_path);
    cmd.arg("run");

    // Add preload if SDK exists
    if let Some(ref sdk) = sdk_path {
        if let Some(sdk_str) = sdk.to_str() {
            cmd.arg("--preload").arg(sdk_str);
        }
    }

    cmd.arg(temp_file.path());

    if let Some(ref cwd) = options.cwd {
        cmd.current_dir(cwd);
    }
    apply_scriptlet_environment(&mut cmd, options);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute TypeScript: {}", e))?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Execute transform scriptlet (get selected text, process, set selected text)
#[cfg(target_os = "macos")]
pub fn execute_transform(
    content: &str,
    options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)
        .map_err(|error| error.to_string())?;
    info!(category = "EXEC", "Executing transform scriptlet");

    // Get selected text
    let selected = selected_text::get_selected_text()
        .map_err(|e| format!("Failed to get selected text: {}", e))?;

    // Create script that processes the input
    // Wrap content in a function that receives selectedText and returns transformed text
    let wrapper_script = format!(
        r#"
const selectedText = {};
const transform = (text: string): string => {{
{}
}};
const result = transform(selectedText);
console.log(result);
"#,
        serde_json::to_string(&selected).unwrap_or_else(|_| "\"\"".to_string()),
        content
    );

    // Execute the transform script
    let result = execute_typescript(&wrapper_script, options)?;

    if result.success {
        // Set the transformed text back
        let transformed = result.stdout.trim();
        selected_text::set_selected_text(transformed)
            .map_err(|e| format!("Failed to set selected text: {}", e))?;
    }

    Ok(result)
}

#[cfg(not(target_os = "macos"))]
pub fn execute_transform(
    _content: &str,
    _options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    Err("Transform scriptlets are only supported on macOS".to_string())
}

#[cfg(any(test, target_os = "windows"))]
fn escape_windows_cmd_launch_argument(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '^' | '&' | '|' | '<' | '>' | '(' | ')' | '%' | '!' | '"' => {
                escaped.push('^');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn build_open_command(target: &str) -> Command {
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg(target);
        command
    }

    #[cfg(target_os = "linux")]
    {
        let mut command = Command::new("xdg-open");
        command.arg(target);
        command
    }

    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command
            .args(["/C", "start", ""])
            .arg(escape_windows_cmd_launch_argument(target));
        command
    }
}

/// Execute open command (open URL or file)
pub fn execute_open(
    content: &str,
    options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal)
        .map_err(|error| error.to_string())?;
    info!(
        category = "EXEC",
        target = %content.trim(),
        "Opening target"
    );

    let target = content.trim();
    let mut cmd = build_open_command(target);
    apply_scriptlet_environment(&mut cmd, options);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to open '{}': {}", target, e))?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod open_command_tests {
    #[cfg(target_os = "windows")]
    use super::build_open_command;
    use super::escape_windows_cmd_launch_argument;

    #[test]
    fn test_escape_windows_cmd_launch_argument_escapes_shell_metacharacters() {
        let escaped = escape_windows_cmd_launch_argument(r#"a&b|c<d>e(f)g^h%i!j"k"#);
        assert_eq!(escaped, r#"a^&b^|c^<d^>e^(f^)g^^h^%i^!j^"k"#);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_build_open_command_uses_cmd_start_with_empty_title_on_windows() {
        let command = build_open_command("https://example.com/?x=1&y=2");
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert_eq!(command.get_program().to_string_lossy(), "cmd");
        assert_eq!(
            args,
            vec!["/C", "start", "", "https://example.com/?x=1^&y=2"]
        );
    }
}

/// Execute edit command (open file in editor)
pub fn execute_edit(
    content: &str,
    options: &ScriptletExecOptions,
) -> Result<ScriptletResult, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal)
        .map_err(|error| error.to_string())?;
    info!(
        category = "EXEC",
        target = %content.trim(),
        "Editing target"
    );

    let file_path = content.trim();

    // Get editor from environment or default
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "code".to_string());

    // Find the editor executable
    let editor_path = find_executable(&editor)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or(editor);

    let mut cmd = Command::new(&editor_path);
    cmd.arg(file_path);
    apply_scriptlet_environment(&mut cmd, options);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to open editor '{}': {}", editor_path, e))?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Execute paste command (set selected text via clipboard)
#[cfg(target_os = "macos")]
pub fn execute_paste(content: &str) -> Result<ScriptletResult, String> {
    info!(category = "EXEC", "Executing paste scriptlet");

    let text = content.trim();

    selected_text::set_selected_text(text).map_err(|e| format!("Failed to paste text: {}", e))?;

    Ok(ScriptletResult {
        exit_code: 0,
        stdout: String::new(),
        stderr: String::new(),
        success: true,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn execute_paste(_content: &str) -> Result<ScriptletResult, String> {
    Err("Paste scriptlets are only supported on macOS".to_string())
}

/// Build the AppleScript that types `text` via System Events.
///
/// The structural quotes around the application name and the `keystroke`
/// argument MUST be real `"` characters. A previous raw-string literal emitted
/// literal backslash-quote (`\"`) sequences, which AppleScript rejects with
/// `syntax error … (-2741)` — so `type` scriptlets silently never typed
/// anything. Payload quotes are AppleScript-escaped (`\"`) via
/// `escape_applescript_string`, which is the only place a backslash-quote should
/// legitimately appear.
#[cfg(target_os = "macos")]
fn type_keystroke_applescript(text: &str) -> String {
    format!(
        r#"tell application "System Events" to keystroke "{}""#,
        crate::utils::escape_applescript_string(text)
    )
}

/// Execute type command (simulate keyboard typing)
#[cfg(target_os = "macos")]
pub fn execute_type(content: &str) -> Result<ScriptletResult, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput)
        .map_err(|error| error.to_string())?;
    info!(category = "EXEC", "Executing type scriptlet");

    let text = content.trim();

    // Use AppleScript to simulate typing
    let script = type_keystroke_applescript(text);

    let mut cmd = Command::new("osascript");
    cmd.arg("-e").arg(&script);
    apply_scriptlet_environment_allowlist(&mut cmd);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to type text: {}", e))?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

#[cfg(not(target_os = "macos"))]
pub fn execute_type(_content: &str) -> Result<ScriptletResult, String> {
    Err("Type scriptlets are only supported on macOS".to_string())
}

/// Execute submit command (paste + enter)
#[cfg(target_os = "macos")]
pub fn execute_submit(content: &str) -> Result<ScriptletResult, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput)
        .map_err(|error| error.to_string())?;
    info!(category = "EXEC", "Executing submit scriptlet");

    // First paste the text
    let paste_result = execute_paste(content)?;
    if !paste_result.success {
        return Ok(paste_result);
    }

    // Small delay to let paste complete
    std::thread::sleep(Duration::from_millis(50));

    // Then press Enter using AppleScript
    let mut cmd = Command::new("osascript");
    cmd.arg("-e")
        .arg(r#"tell application "System Events" to key code 36"#); // 36 is Return key
    apply_scriptlet_environment_allowlist(&mut cmd);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to press Enter: {}", e))?;

    Ok(ScriptletResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

#[cfg(not(target_os = "macos"))]
pub fn execute_submit(_content: &str) -> Result<ScriptletResult, String> {
    Err("Submit scriptlets are only supported on macOS".to_string())
}

#[cfg(all(test, target_os = "macos"))]
mod type_keystroke_tests {
    use super::type_keystroke_applescript;

    #[test]
    fn type_keystroke_applescript_uses_real_structural_quotes() {
        // Regression: the builder used a raw-string literal with escaped quotes
        // (`\"`), producing `tell application \"System Events\" to keystroke \"hi\"`
        // — which AppleScript rejects with syntax error (-2741), so `type`
        // scriptlets silently typed nothing. With a payload that has no quotes,
        // the generated script must contain NO literal backslash-quote at all.
        let script = type_keystroke_applescript("hi");
        assert!(
            !script.contains("\\\""),
            "structural quotes must be real, not escaped: {script}"
        );
        assert_eq!(
            script,
            r#"tell application "System Events" to keystroke "hi""#
        );
    }

    #[test]
    fn type_keystroke_applescript_escapes_payload_quotes() {
        // A quote *inside* the typed text is the one place a backslash-quote is
        // correct — it is the AppleScript escape for an embedded `"`.
        let script = type_keystroke_applescript(r#"a"b"#);
        assert_eq!(
            script,
            r#"tell application "System Events" to keystroke "a\"b""#
        );
    }
}

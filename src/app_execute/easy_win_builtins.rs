const SCRIPT_KIT_MCP_TOKEN_ENV: &str = "SCRIPT_KIT_MCP_TOKEN";

struct CodexMcpDiscovery {
    endpoint: String,
    token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExampleScriptInstallReport {
    installed: usize,
    skipped: usize,
}

fn script_kit_workspace_root() -> Result<std::path::PathBuf, String> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(std::path::PathBuf::from)
        .map(|home| home.join(".scriptkit"))
        .ok_or_else(|| "HOME is not available; cannot locate the Script Kit workspace".to_string())
}

fn normalize_script_kit_mcp_endpoint(base_url: &str) -> Result<String, String> {
    let base_url = base_url.trim();
    if base_url.is_empty() {
        return Err("Script Kit MCP discovery is missing url".to_string());
    }
    if base_url.chars().any(char::is_whitespace) {
        return Err("Script Kit MCP discovery url contains whitespace".to_string());
    }
    if base_url.contains('?') || base_url.contains('#') {
        return Err("Script Kit MCP discovery url must not contain a query or fragment".to_string());
    }
    let authority_and_path = base_url
        .strip_prefix("http://")
        .or_else(|| base_url.strip_prefix("https://"))
        .ok_or_else(|| "Script Kit MCP discovery url must use http or https".to_string())?;
    if authority_and_path.split('/').next().unwrap_or_default().is_empty() {
        return Err("Script Kit MCP discovery url is missing a host".to_string());
    }

    let base_url = base_url.trim_end_matches('/');
    let endpoint = if base_url.ends_with("/rpc") {
        base_url.to_string()
    } else {
        format!("{base_url}/rpc")
    };
    if !endpoint.ends_with("/rpc") {
        return Err("Script Kit MCP endpoint must end in /rpc".to_string());
    }

    Ok(endpoint)
}

fn build_codex_mcp_add_argv(endpoint: &str) -> Result<Vec<String>, String> {
    if !endpoint.ends_with("/rpc") {
        return Err("Script Kit MCP endpoint must end in /rpc".to_string());
    }

    Ok(vec![
        "codex".to_string(),
        "mcp".to_string(),
        "add".to_string(),
        "script-kit".to_string(),
        "--url".to_string(),
        endpoint.to_string(),
        "--bearer-token-env-var".to_string(),
        SCRIPT_KIT_MCP_TOKEN_ENV.to_string(),
    ])
}

fn read_codex_mcp_discovery(path: &std::path::Path) -> Result<CodexMcpDiscovery, String> {
    let raw = std::fs::read_to_string(path).map_err(|error| {
        format!(
            "Script Kit MCP discovery is unavailable at {}: {error}",
            path.display()
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|error| format!("Script Kit MCP discovery is invalid JSON: {error}"))?;
    let base_url = value
        .get("url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Script Kit MCP discovery is missing url".to_string())?;
    let token = value
        .get("token")
        .and_then(serde_json::Value::as_str)
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| "Script Kit MCP discovery is missing token".to_string())?
        .to_string();

    Ok(CodexMcpDiscovery {
        endpoint: normalize_script_kit_mcp_endpoint(base_url)?,
        token,
    })
}

fn register_codex_mcp_from_server_json(path: &std::path::Path) -> Result<String, String> {
    let discovery = read_codex_mcp_discovery(path)?;
    let argv = build_codex_mcp_add_argv(&discovery.endpoint)?;
    let status = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .env(SCRIPT_KIT_MCP_TOKEN_ENV, &discovery.token)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "Codex CLI is not available on PATH".to_string()
            } else {
                format!("Failed to launch Codex MCP registration: {error}")
            }
        })?;
    if !status.success() {
        return Err(format!(
            "Codex MCP registration failed with status {status}"
        ));
    }

    Ok(discovery.endpoint)
}

fn is_example_script_path(path: &std::path::Path) -> bool {
    let Some(extension) = path.extension().and_then(std::ffi::OsStr::to_str) else {
        return false;
    };
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs"
    )
}

fn create_example_script_skip_existing(
    destination: &std::path::Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<bool, String> {
    let mut destination_file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
        Err(error) => {
            return Err(format!(
                "Failed to create example script {}: {error}",
                destination.display()
            ));
        }
    };

    if let Err(error) = write(&mut destination_file) {
        drop(destination_file);
        let _ = std::fs::remove_file(destination);
        return Err(format!(
            "Failed to write example script {}: {error}",
            destination.display()
        ));
    }

    Ok(true)
}

fn copy_example_scripts_skip_existing(
    source_root: &std::path::Path,
    destination_root: &std::path::Path,
) -> Result<ExampleScriptInstallReport, String> {
    std::fs::create_dir_all(destination_root).map_err(|error| {
        format!(
            "Failed to create main scripts directory {}: {error}",
            destination_root.display()
        )
    })?;

    let entries = std::fs::read_dir(source_root).map_err(|error| {
        format!(
            "Failed to read example script directory {}: {error}",
            source_root.display()
        )
    })?;
    let mut source_paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("Failed to enumerate example scripts: {error}"))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!("Failed to inspect example script {}: {error}", path.display())
        })?;
        if file_type.is_file() && is_example_script_path(&path) {
            source_paths.push(path);
        }
    }
    source_paths.sort();

    let mut report = ExampleScriptInstallReport {
        installed: 0,
        skipped: 0,
    };
    for source in source_paths {
        let file_name = source
            .file_name()
            .ok_or_else(|| format!("Example script has no file name: {}", source.display()))?;
        let destination = destination_root.join(file_name);
        let mut source_file = std::fs::File::open(&source).map_err(|error| {
            format!("Failed to open example script {}: {error}", source.display())
        })?;
        if create_example_script_skip_existing(&destination, |destination_file| {
            std::io::copy(&mut source_file, destination_file).map(|_| ())
        })? {
            report.installed += 1;
        } else {
            report.skipped += 1;
        }
    }

    Ok(report)
}

fn install_example_scripts(
    workspace_root: &std::path::Path,
) -> Result<ExampleScriptInstallReport, String> {
    let destination_root = workspace_root.join("plugins/main/scripts");
    let source_candidates = [
        workspace_root.join("plugins/examples/scripts"),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("kit-init/examples/scripts"),
    ];

    for source_root in source_candidates {
        if !source_root.is_dir() {
            continue;
        }
        let report = copy_example_scripts_skip_existing(&source_root, &destination_root)?;
        if report.installed + report.skipped > 0 {
            return Ok(report);
        }
    }

    Err("No bundled example scripts were found".to_string())
}

#[cfg(test)]
mod easy_win_builtins_tests {
    use super::*;

    fn unique_test_directory(label: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "script-kit-easy-win-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn codex_mcp_argv_uses_rpc_url_and_token_environment_reference() {
        let endpoint = normalize_script_kit_mcp_endpoint("http://127.0.0.1:43210/")
            .expect("base server url should produce an rpc endpoint");
        let argv = build_codex_mcp_add_argv(&endpoint)
            .expect("rpc endpoint should produce Codex registration argv");

        assert!(
            build_codex_mcp_add_argv("http://127.0.0.1:43210").is_err(),
            "argv builder must require the /rpc endpoint"
        );
        assert_eq!(endpoint, "http://127.0.0.1:43210/rpc");
        assert_eq!(
            argv,
            vec![
                "codex".to_string(),
                "mcp".to_string(),
                "add".to_string(),
                "script-kit".to_string(),
                "--url".to_string(),
                "http://127.0.0.1:43210/rpc".to_string(),
                "--bearer-token-env-var".to_string(),
                "SCRIPT_KIT_MCP_TOKEN".to_string(),
            ]
        );
        assert!(
            argv.iter().all(|argument| argument != "secret-token-value"),
            "the bearer token literal must never enter Codex argv"
        );
    }

    #[test]
    fn example_script_copy_skips_existing_destination_without_overwrite() {
        let root = unique_test_directory("copy");
        let source_root = root.join("source");
        let destination_root = root.join("destination");
        std::fs::create_dir_all(&source_root).expect("source directory should be created");
        std::fs::create_dir_all(&destination_root)
            .expect("destination directory should be created");
        std::fs::write(source_root.join("keep.ts"), b"bundled keep")
            .expect("bundled keep script should be written");
        std::fs::write(source_root.join("new.ts"), b"bundled new")
            .expect("bundled new script should be written");
        std::fs::write(destination_root.join("keep.ts"), b"user owned")
            .expect("user-owned script should be written");

        let report = copy_example_scripts_skip_existing(&source_root, &destination_root)
            .expect("example script copy should succeed");

        assert_eq!(
            report,
            ExampleScriptInstallReport {
                installed: 1,
                skipped: 1,
            }
        );
        assert_eq!(
            std::fs::read(destination_root.join("keep.ts"))
                .expect("existing destination should still be readable"),
            b"user owned",
            "existing user script bytes must not be overwritten"
        );
        assert_eq!(
            std::fs::read(destination_root.join("new.ts"))
                .expect("new destination should be readable"),
            b"bundled new"
        );

        std::fs::remove_dir_all(root).expect("test directory should be removed");
    }
}

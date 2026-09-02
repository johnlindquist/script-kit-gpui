use anyhow::Context as _;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::runtime::Runtime;

#[cfg(target_os = "macos")]
static OSASCRIPT_RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|err| panic!("Failed to create tokio runtime for osascript: {err}"))
});

#[cfg(target_os = "macos")]
pub fn run_osascript(script: &str, context: &str) -> anyhow::Result<String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)?;
    tracing::debug!(
        context = context,
        script_len = script.len(),
        "running osascript"
    );

    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .with_context(|| {
            format!(
                "platform_run_osascript_failed: stage=spawn context={} attempt=osascript -e",
                context
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        tracing::error!(
            context = context,
            status = %output.status,
            stderr = %stderr,
            "osascript exited with non-zero status"
        );
        anyhow::bail!(
            "platform_run_osascript_failed: stage=exit_status context={} status={} stderr={}",
            context,
            output.status,
            if stderr.is_empty() {
                "<empty>"
            } else {
                stderr.as_str()
            }
        );
    }

    tracing::debug!(
        context = context,
        status = %output.status,
        stdout_len = stdout.len(),
        "osascript completed"
    );

    Ok(stdout)
}

#[cfg(target_os = "macos")]
pub fn run_osascript_with_timeout(
    script: &str,
    context: &str,
    timeout: Duration,
) -> anyhow::Result<String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)?;
    tracing::debug!(
        context = context,
        script_len = script.len(),
        timeout = ?timeout,
        "running osascript with timeout"
    );

    OSASCRIPT_RUNTIME.block_on(async {
        let child = tokio::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| {
                format!(
                    "platform_run_osascript_failed: stage=spawn context={} attempt=osascript -e",
                    context
                )
            })?;

        match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

                if !output.status.success() {
                    tracing::error!(
                        context = context,
                        status = %output.status,
                        stderr = %stderr,
                        "osascript exited with non-zero status"
                    );
                    anyhow::bail!(
                        "platform_run_osascript_failed: stage=exit_status context={} status={} stderr={}",
                        context,
                        output.status,
                        if stderr.is_empty() {
                            "<empty>"
                        } else {
                            stderr.as_str()
                        }
                    );
                }
                Ok(stdout)
            }
            Ok(Err(e)) => Err(anyhow::Error::from(e).context(format!(
                "platform_run_osascript_failed: stage=execution context={}",
                context
            ))),
            Err(_) => {
                anyhow::bail!(
                    "platform_run_osascript_failed: stage=timeout context={} timeout={:?}",
                    context,
                    timeout
                )
            }
        }
    })
}

#[cfg(target_os = "macos")]
pub fn run_jxa(script: &str, context: &str) -> anyhow::Result<String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)?;
    tracing::debug!(
        context = context,
        script_len = script.len(),
        "running osascript (JXA)"
    );

    let output = std::process::Command::new("osascript")
        .arg("-l")
        .arg("JavaScript")
        .arg("-e")
        .arg(script)
        .output()
        .with_context(|| {
            format!(
                "platform_run_jxa_failed: stage=spawn context={} attempt=osascript -l JavaScript -e",
                context
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        tracing::error!(
            context = context,
            status = %output.status,
            stderr = %stderr,
            "osascript (JXA) exited with non-zero status"
        );
        anyhow::bail!(
            "platform_run_jxa_failed: stage=exit_status context={} status={} stderr={}",
            context,
            output.status,
            if stderr.is_empty() {
                "<empty>"
            } else {
                stderr.as_str()
            }
        );
    }

    tracing::debug!(
        context = context,
        status = %output.status,
        stdout_len = stdout.len(),
        "osascript (JXA) completed"
    );

    Ok(stdout)
}

#[cfg(target_os = "macos")]
pub fn run_jxa_with_timeout(
    script: &str,
    context: &str,
    timeout: Duration,
) -> anyhow::Result<String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)?;
    tracing::debug!(
        context = context,
        script_len = script.len(),
        timeout = ?timeout,
        "running JXA with timeout"
    );

    OSASCRIPT_RUNTIME.block_on(async {
        let child = tokio::process::Command::new("osascript")
            .arg("-l")
            .arg("JavaScript")
            .arg("-e")
            .arg(script)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| {
                format!(
                    "platform_run_jxa_failed: stage=spawn context={} attempt=osascript -l JavaScript -e",
                    context
                )
            })?;

        match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

                if !output.status.success() {
                    tracing::error!(
                        context = context,
                        status = %output.status,
                        stderr = %stderr,
                        "JXA exited with non-zero status"
                    );
                    anyhow::bail!(
                        "platform_run_jxa_failed: stage=exit_status context={} status={} stderr={}",
                        context,
                        output.status,
                        if stderr.is_empty() {
                            "<empty>"
                        } else {
                            stderr.as_str()
                        }
                    );
                }
                Ok(stdout)
            }
            Ok(Err(e)) => Err(anyhow::Error::from(e).context(format!(
                "platform_run_jxa_failed: stage=execution context={}",
                context
            ))),
            Err(_) => {
                anyhow::bail!(
                    "platform_run_jxa_failed: stage=timeout context={} timeout={:?}",
                    context,
                    timeout
                )
            }
        }
    })
}

#[cfg(not(target_os = "macos"))]
pub fn run_jxa(_script: &str, context: &str) -> anyhow::Result<String> {
    anyhow::bail!(
        "platform_run_jxa_failed: stage=unsupported_platform context={} platform={}",
        context,
        std::env::consts::OS
    );
}

#[cfg(not(target_os = "macos"))]
pub fn run_osascript(_script: &str, context: &str) -> anyhow::Result<String> {
    anyhow::bail!(
        "platform_run_osascript_failed: stage=unsupported_platform context={} platform={}",
        context,
        std::env::consts::OS
    );
}

#[cfg(not(target_os = "macos"))]
pub fn run_osascript_with_timeout(
    _script: &str,
    context: &str,
    _timeout: Duration,
) -> anyhow::Result<String> {
    anyhow::bail!(
        "platform_run_osascript_failed: stage=unsupported_platform context={} platform={}",
        context,
        std::env::consts::OS
    );
}

#[cfg(not(target_os = "macos"))]
pub fn run_jxa_with_timeout(
    _script: &str,
    context: &str,
    _timeout: Duration,
) -> anyhow::Result<String> {
    anyhow::bail!(
        "platform_run_jxa_failed: stage=unsupported_platform context={} platform={}",
        context,
        std::env::consts::OS
    );
}

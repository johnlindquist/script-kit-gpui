/// Launch an application
///
/// Uses macOS `open -a` command to launch the application.
///
/// # Arguments
/// * `app` - The application to launch
///
/// # Returns
/// Ok(()) if the application was launched successfully, Err otherwise.
///
pub fn launch_application(app: &AppInfo) -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal)?;
    info!(
        app_name = %app.name,
        app_path = %app.path.display(),
        "Launching application"
    );

    // A stale cache entry (moved/uninstalled app) would otherwise "succeed":
    // spawn() only reports that /usr/bin/open started, not that the app launched.
    if !app.path.exists() {
        anyhow::bail!(
            "{} is no longer at {} — it may have been moved or uninstalled",
            app.name,
            app.path.display()
        );
    }

    let mut child = Command::new("open")
        .arg("-a")
        .arg(&app.path)
        .spawn()
        .with_context(|| format!("Failed to launch application: {}", app.name))?;

    // `open` exits promptly; observe its status off-thread so a failed launch
    // (damaged/translocated bundle) at least leaves an error trail instead of
    // nothing, and the child never lingers as a zombie.
    let app_name = app.name.clone();
    std::thread::spawn(move || match child.wait() {
        Ok(status) if !status.success() => {
            tracing::error!(
                app_name = %app_name,
                exit_code = ?status.code(),
                "`open -a` exited non-zero; application failed to launch"
            );
        }
        Err(error) => {
            tracing::error!(
                app_name = %app_name,
                error = %error,
                "Failed to wait on `open -a` child process"
            );
        }
        _ => {}
    });

    Ok(())
}

/// Launch an application by name
///
/// Convenience function that looks up an application by name and launches it.
///
/// # Arguments
/// * `name` - The name of the application (case-insensitive)
///
/// # Returns
/// Ok(()) if the application was found and launched, Err otherwise.
#[allow(dead_code)]
pub fn launch_application_by_name(name: &str) -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal)?;
    let apps = scan_applications()?;
    let name_lower = name.to_lowercase();

    let app = apps
        .iter()
        .find(|a| a.name.to_lowercase() == name_lower)
        .with_context(|| format!("Application not found: {}", name))?;

    launch_application(app)
}

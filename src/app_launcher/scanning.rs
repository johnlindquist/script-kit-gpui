// ============================================================================
// Application Scanning
// ============================================================================

/// Scan for installed macOS applications
///
/// This function uses a two-phase loading strategy:
/// 1. Load the last SQLite snapshot, including decoded icons, if available.
/// 2. Refresh it in the background; failures retain the snapshot and are reported.
///
/// # Returns
/// An owned cached snapshot, or the initialization/most recent scan error.
///
/// # Performance
/// - First call: Reads SQLite and, without a cached snapshot, scans directories.
/// - Subsequent calls: Returns immediately from in-memory cache
///
/// # Tracing
/// Uses spans to profile: db_lock, query, deserialization, icon_decode
pub fn scan_applications() -> Result<Vec<AppInfo>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    let _span = info_span!("scan_applications").entered();
    let result = (|| {
        if let Some(apps) = app_cache_snapshot(&APP_CACHE)? {
            return Ok(apps);
        }
        let _scan = APP_SCAN_LOCK
            .lock()
            .map_err(|_| anyhow::anyhow!("Application scan lock poisoned"))?;
        if let Some(apps) = app_cache_snapshot(&APP_CACHE)? {
            return Ok(apps);
        }
        set_loading_state(AppLoadingState::LoadingFromCache);
        let cached_apps = match load_apps_from_db() {
            Ok(apps) => apps,
            Err(error) => return complete_app_scan(&APP_CACHE, Err(error)),
        };
        if cached_apps.is_empty() {
            return refresh_app_cache();
        }
        let apps = complete_app_scan(&APP_CACHE, Ok(cached_apps))?;
        set_loading_state(AppLoadingState::ScanningDirectories);
        if let Err(error) = std::thread::Builder::new()
            .name("application-cache-refresh".into())
            .spawn(|| {
                if let Err(error) = scan_applications_fresh() {
                    warn!(%error, "Application refresh failed; retaining last successful cache");
                }
            })
        {
            return complete_app_scan(
                &APP_CACHE,
                Err(error).context("Starting application cache refresh"),
            );
        }
        Ok(apps)
    })();
    if result.is_err() {
        set_loading_state(AppLoadingState::Failed);
    }
    result
}

/// Force a fresh scan of applications and replace the in-memory cache.
///
/// This is how newly installed/removed apps show up without an app restart:
/// the app watcher calls this on /Applications changes. Blocking (disk +
/// sqlite) — run on a background thread/executor, never the UI thread.
pub fn scan_applications_fresh() -> Result<Vec<AppInfo>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    let _scan = APP_SCAN_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("Application scan lock poisoned"))?;
    refresh_app_cache()
}

fn refresh_app_cache() -> Result<Vec<AppInfo>> {
    set_loading_state(AppLoadingState::ScanningDirectories);
    let start = Instant::now();
    let result = complete_app_scan(&APP_CACHE, scan_all_directories_with_db_update());
    match &result {
        Ok(apps) => {
            set_loading_state(AppLoadingState::Ready);
            info!(
                app_count = apps.len(),
                duration_ms = start.elapsed().as_millis(),
                "Fresh scan of applications (cache updated)"
            );
        }
        Err(_) => set_loading_state(AppLoadingState::Failed),
    }
    result
}

/// Reset icon extraction stats before a new scan
fn reset_icon_stats() {
    ICONS_EXTRACTED.store(0, Ordering::Relaxed);
    ICONS_FROM_CACHE.store(0, Ordering::Relaxed);
    ICONS_FROM_BUNDLE_RESOURCE.store(0, Ordering::Relaxed);
    ICONS_FROM_ICON_SERVICES.store(0, Ordering::Relaxed);
    ICONS_SKIPPED_ICON_SERVICES.store(0, Ordering::Relaxed);
    EXTRACT_TIME_MS.store(0, Ordering::Relaxed);
}

/// Log a summary of icon extraction stats
fn log_icon_stats_summary() {
    let extracted = ICONS_EXTRACTED.load(Ordering::Relaxed);
    let from_cache = ICONS_FROM_CACHE.load(Ordering::Relaxed);
    let from_bundle_resource = ICONS_FROM_BUNDLE_RESOURCE.load(Ordering::Relaxed);
    let from_icon_services = ICONS_FROM_ICON_SERVICES.load(Ordering::Relaxed);
    let skipped_icon_services = ICONS_SKIPPED_ICON_SERVICES.load(Ordering::Relaxed);
    let total_ms = EXTRACT_TIME_MS.load(Ordering::Relaxed);

    if extracted > 0 || from_cache > 0 || skipped_icon_services > 0 {
        info!(
            icons_extracted = extracted,
            icons_from_cache = from_cache,
            icons_from_bundle_resource = from_bundle_resource,
            icons_from_icon_services = from_icon_services,
            icons_skipped_icon_services = skipped_icon_services,
            total_extract_ms = total_ms,
            avg_extract_ms = total_ms.checked_div(extracted).unwrap_or(0),
            "Icon extraction summary"
        );
    }
}

/// Scan all configured directories for applications and update SQLite
fn scan_all_directories_with_db_update() -> Result<Vec<AppInfo>> {
    let _span = info_span!("scan_all_directories_with_db_update").entered();
    let start = Instant::now();
    reset_icon_stats();
    let roots: Vec<PathBuf> = APP_DIRECTORIES
        .iter()
        .map(|dir| PathBuf::from(shellexpand::tilde(dir).as_ref()))
        .collect();
    // Finish enumeration and parsing before touching SQLite: a failed root must
    // not turn the successful prefix into either a published or persisted scan.
    let app_paths = collect_app_paths_from_roots(&roots)?;
    let mut scanned: Vec<ScannedApp> = app_paths
        .par_iter()
        .map(|path| -> Result<Option<ScannedApp>> {
            let Some((app, icon_bytes)) = parse_app_bundle_with_icon(path)? else {
                return Ok(None);
            };
            let mtime = get_mtime(path)?;
            Ok(Some(ScannedApp {
                app,
                icon_bytes,
                mtime,
            }))
        })
        .filter_map(Result::transpose)
        .collect::<Result<Vec<_>>>()?;
    scanned.sort_by_cached_key(|entry| entry.app.name.to_lowercase());
    scanned.dedup_by(|a, b| a.app.name.to_lowercase() == b.app.name.to_lowercase());
    let db = get_apps_db()?;
    let mut conn = db
        .lock()
        .map_err(|_| anyhow::anyhow!("Application database lock poisoned"))?;
    save_apps_to_db(&mut conn, &scanned)?;
    log_icon_stats_summary();
    debug!(
        total_apps = scanned.len(),
        total_duration_ms = start.elapsed().as_millis(),
        "Directory scan complete"
    );
    Ok(scanned.into_iter().map(|entry| entry.app).collect())
}

struct ScannedApp {
    app: AppInfo,
    icon_bytes: Option<Vec<u8>>,
    mtime: i64,
}

fn collect_app_paths_from_roots(roots: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for root in roots {
        // These installation locations are optional across macOS versions and
        // user setups. Only genuine absence is optional, never a failed read.
        match fs::read_dir(root) {
            Ok(entries) => collect_app_entries(root, entries, &mut paths)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to read application root: {}", root.display())
                })
            }
        }
    }
    Ok(paths)
}

/// Parse a .app bundle to extract application information and icon bytes
fn parse_app_bundle_with_icon(path: &Path) -> Result<Option<(AppInfo, Option<Vec<u8>>)>> {
    // A bundle whose filename cannot be represented as an application name is
    // an invalid individual entry, not a failed directory scan.
    let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
        return Ok(None);
    };
    let name = name.to_owned();

    // Try to extract bundle identifier from Info.plist
    let bundle_id = extract_bundle_id(path)?;

    // Extract icon (macOS only)
    #[cfg(target_os = "macos")]
    let icon_bytes = get_or_extract_icon(path)?;
    #[cfg(not(target_os = "macos"))]
    let icon_bytes: Option<Vec<u8>> = None;

    // Pre-decode icon for rendering
    let icon = icon_bytes.as_ref().and_then(|bytes| {
        crate::list_item::decode_png_to_render_image_with_bgra_conversion(bytes).ok()
    });

    Ok(Some((
        AppInfo {
            name,
            path: path.to_path_buf(),
            bundle_id,
            icon,
        },
        icon_bytes,
    )))
}

fn collect_app_paths_recursive(dir: &Path, app_paths: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(dir)
        .with_context(|| format!("Failed to read application directory: {}", dir.display()))?;
    collect_app_entries(dir, entries, app_paths)
}

fn collect_app_entries(
    dir: &Path,
    entries: fs::ReadDir,
    app_paths: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "Failed to enumerate application directory: {}",
                dir.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to read application entry: {}", path.display()))?;
        if path.extension().is_some_and(|extension| extension == "app") {
            if file_type.is_dir() || file_type.is_symlink() {
                app_paths.push(path);
            }
        } else if file_type.is_dir() {
            collect_app_paths_recursive(&path, app_paths)?;
        }
    }
    Ok(())
}

/// Extract CFBundleIdentifier from Info.plist
///
/// Uses /usr/libexec/PlistBuddy for reliable plist parsing.
fn extract_bundle_id(app_path: &Path) -> Result<Option<String>> {
    plist_value(&app_path.join("Contents/Info.plist"), ":CFBundleIdentifier")
}

fn icon_extraction_disabled() -> bool {
    std::env::var("SCRIPT_KIT_DISABLE_APP_ICON_EXTRACTION")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn icon_services_fallback_enabled() -> bool {
    std::env::var("SCRIPT_KIT_ENABLE_ICON_SERVICES_FALLBACK")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn plist_value(plist_path: &Path, key_path: &str) -> Result<Option<String>> {
    match fs::metadata(plist_path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Ok(None), // Invalid individual bundle metadata is optional.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Reading application plist: {}", plist_path.display()))
        }
    }
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", &format!("Print {key_path}")])
        .arg(plist_path)
        .output();
    decode_plist_output(output, key_path)
        .with_context(|| format!("Reading {key_path} from {}", plist_path.display()))
}

fn decode_plist_output(
    output: std::io::Result<std::process::Output>,
    key_path: &str,
) -> Result<Option<String>> {
    let output = output.context("Running PlistBuddy")?;
    let stdout = String::from_utf8(output.stdout).context("PlistBuddy returned invalid UTF-8")?;
    let stderr =
        String::from_utf8(output.stderr).context("PlistBuddy returned invalid diagnostics")?;
    if !output.status.success() {
        // PlistBuddy's missing-key diagnostic is optional metadata. Every other
        // failed command, including a file read failure, remains a source error.
        let missing = format!("Print: Entry, \"{key_path}\", Does Not Exist");
        let mut diagnostics = stdout
            .lines()
            .chain(stderr.lines())
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .peekable();
        if diagnostics.peek().is_some() && diagnostics.all(|line| line == missing) {
            return Ok(None);
        }
        anyhow::bail!(
            "PlistBuddy failed ({}): {} {}",
            output.status,
            stdout.trim(),
            stderr.trim()
        );
    }
    let value = stdout.trim();
    Ok((!value.is_empty()).then(|| value.to_owned()))
}

fn icon_file_candidates(icon_name: &str) -> [String; 2] {
    if icon_name.ends_with(".icns") {
        [
            icon_name.to_string(),
            icon_name.trim_end_matches(".icns").to_string(),
        ]
    } else {
        [format!("{icon_name}.icns"), icon_name.to_string()]
    }
}

fn resolve_bundle_icon_resource_path(app_path: &Path) -> Result<Option<PathBuf>> {
    let resources_dir = app_path.join("Contents/Resources");
    match fs::metadata(&resources_dir) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| {
                format!("Reading application resources: {}", resources_dir.display())
            })
        }
    }
    let plist_path = app_path.join("Contents/Info.plist");
    for key_path in [
        ":CFBundleIconFile",
        ":CFBundleIcons:CFBundlePrimaryIcon:CFBundleIconFiles:0",
        ":CFBundleIcons:CFBundlePrimaryIcon:CFBundleIconName",
    ] {
        let Some(icon_name) = plist_value(&plist_path, key_path)? else {
            continue;
        };
        for candidate in icon_file_candidates(&icon_name) {
            let icon_path = resources_dir.join(candidate);
            match fs::metadata(&icon_path) {
                Ok(metadata) if metadata.is_file() => return Ok(Some(icon_path)),
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("Reading application icon resource: {}", icon_path.display())
                    })
                }
            }
        }
    }
    for entry in fs::read_dir(&resources_dir)
        .with_context(|| format!("Reading application resources: {}", resources_dir.display()))?
    {
        let path = entry.context("Reading application resource entry")?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "icns")
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
fn image_to_png_bytes(image: id) -> Option<Vec<u8>> {
    use std::slice;

    if image == nil {
        return None;
    }

    // Set the icon size to 32x32 for list display
    let size = cocoa::foundation::NSSize::new(32.0, 32.0);
    let _: () = unsafe { msg_send![image, setSize: size] };

    // Get TIFF representation
    let tiff_data: id = unsafe { msg_send![image, TIFFRepresentation] };
    if tiff_data == nil {
        return None;
    }

    // Create bitmap image rep from TIFF data
    let bitmap_rep: id =
        unsafe { msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff_data] };
    if bitmap_rep == nil {
        return None;
    }

    // Convert to PNG (NSPNGFileType = 4)
    let empty_dict: id = unsafe { msg_send![class!(NSDictionary), dictionary] };
    let png_data: id = unsafe {
        msg_send![
            bitmap_rep,
            representationUsingType: 4u64  // NSPNGFileType
            properties: empty_dict
        ]
    };
    if png_data == nil {
        return None;
    }

    // Get bytes from NSData
    let length: usize = unsafe { msg_send![png_data, length] };
    let bytes: *const u8 = unsafe { msg_send![png_data, bytes] };

    if bytes.is_null() || length == 0 {
        return None;
    }

    Some(unsafe { slice::from_raw_parts(bytes, length).to_vec() })
}

/// Extract application icon from the bundle's declared icon resource.
///
/// This path avoids NSWorkspace/iconForFile so it does not populate Apple's
/// global IconServices cache at `/Library/Caches/com.apple.iconservices.store`.
#[cfg(target_os = "macos")]
fn extract_app_icon_from_bundle_resource(app_path: &Path) -> Result<Option<Vec<u8>>> {
    let Some(icon_path) = resolve_bundle_icon_resource_path(app_path)? else {
        return Ok(None);
    };
    let bytes = fs::read(&icon_path)
        .with_context(|| format!("Reading application icon: {}", icon_path.display()))?;
    unsafe {
        let data: id = msg_send![class!(NSData), dataWithBytes: bytes.as_ptr().cast::<std::ffi::c_void>() length: bytes.len()];
        anyhow::ensure!(data != nil, "Allocating application icon data failed");
        let image: id = msg_send![class!(NSImage), alloc];
        anyhow::ensure!(image != nil, "Allocating application icon image failed");
        let image: id = msg_send![image, initWithData: data];
        let png_bytes = image_to_png_bytes(image);
        let _: () = msg_send![image, release];
        let png_bytes = png_bytes.with_context(|| {
            format!(
                "Decoding application icon resource: {}",
                icon_path.display()
            )
        })?;
        ICONS_FROM_BUNDLE_RESOURCE.fetch_add(1, Ordering::Relaxed);
        Ok(Some(png_bytes))
    }
}

/// Extract application icon using NSWorkspace.
///
/// Uses macOS Cocoa APIs to get the icon for an application bundle.
/// The icon is converted to PNG format at 32x32 pixels for list display.
/// Returns raw PNG bytes - caller should decode once and cache the RenderImage.
#[cfg(target_os = "macos")]
fn extract_app_icon_via_icon_services(app_path: &Path) -> Result<Option<Vec<u8>>> {
    let path_str = app_path
        .to_str()
        .context("IconServices application path is not UTF-8")?;

    unsafe {
        // Get NSWorkspace shared instance
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            anyhow::bail!("IconServices workspace unavailable");
        }

        // Create NSString for path
        let ns_path = CocoaNSString::alloc(nil).init_str(path_str);
        if ns_path == nil {
            anyhow::bail!("Allocating IconServices application path failed");
        }

        // Get icon for file
        let icon: id = msg_send![workspace, iconForFile: ns_path];
        let _: () = msg_send![ns_path, release];
        let png_bytes =
            image_to_png_bytes(icon).context("Decoding IconServices application icon failed")?;
        ICONS_FROM_ICON_SERVICES.fetch_add(1, Ordering::Relaxed);
        Ok(Some(png_bytes))
    }
}

/// Extract application icon without using IconServices by default.
#[cfg(target_os = "macos")]
fn extract_app_icon(app_path: &Path) -> Result<Option<Vec<u8>>> {
    if icon_extraction_disabled() {
        trace!(
            app = %app_path.display(),
            "Skipping app icon extraction because SCRIPT_KIT_DISABLE_APP_ICON_EXTRACTION is set"
        );
        return Ok(None);
    }

    if let Some(png_bytes) = extract_app_icon_from_bundle_resource(app_path)? {
        return Ok(Some(png_bytes));
    }

    if icon_services_fallback_enabled() {
        return extract_app_icon_via_icon_services(app_path);
    }

    ICONS_SKIPPED_ICON_SERVICES.fetch_add(1, Ordering::Relaxed);
    trace!(
        app = %app_path.display(),
        "Skipping IconServices app icon fallback; set SCRIPT_KIT_ENABLE_ICON_SERVICES_FALLBACK=1 to opt in"
    );
    Ok(None)
}

/// Read optional icons only for the caller's observed application path set.
/// This never scans or replaces the application catalogue.
pub fn read_app_icons(paths: Vec<PathBuf>) -> Result<Vec<(PathBuf, DecodedIcon)>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    #[cfg(target_os = "macos")]
    {
        paths
            .into_par_iter()
            .map(|path| {
                let Some(bytes) = get_or_extract_icon(&path)? else {
                    return Ok(None);
                };
                let icon =
                    crate::list_item::decode_png_to_render_image_with_bgra_conversion(&bytes)
                        .with_context(|| {
                            format!("Decoding application icon: {}", path.display())
                        })?;
                Ok(Some((path, icon)))
            })
            .filter_map(|result: Result<Option<(PathBuf, DecodedIcon)>>| result.transpose())
            .collect()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = paths;
        anyhow::bail!("Application icon discovery is unavailable on this platform")
    }
}

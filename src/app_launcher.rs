//! App Launcher Module
//!
//! Provides functionality to scan and launch macOS applications.
//!
//! ## Features
//! - Scans standard macOS application directories
//! - Caches results for performance (apps don't change often)
//! - Extracts bundle identifiers from Info.plist when available
//! - Launches applications via `open -a`
//!
//! ## Usage
//! ```ignore
//! use crate::app_launcher::{scan_applications, launch_application, AppInfo};
//!
//! // Get all installed applications (cached after first call)
//! let apps = scan_applications();
//!
//! // Launch an application
//! if let Some(app) = apps.iter().find(|a| a.name == "Finder") {
//!     launch_application(app)?;
//! }
//! ```

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Instant;
use tracing::{debug, info, warn};

/// Information about an installed application
#[derive(Debug, Clone)]
pub struct AppInfo {
    /// Display name of the application (e.g., "Safari")
    pub name: String,
    /// Full path to the .app bundle (e.g., "/Applications/Safari.app")
    pub path: PathBuf,
    /// Bundle identifier from Info.plist (e.g., "com.apple.Safari")
    pub bundle_id: Option<String>,
}

/// Cached list of applications (scanned once, reused)
static APP_CACHE: OnceLock<Vec<AppInfo>> = OnceLock::new();

/// Directories to scan for .app bundles
const APP_DIRECTORIES: &[&str] = &[
    "/Applications",
    "/System/Applications",
    "~/Applications",
    "/Applications/Utilities",
];

/// Scan for installed macOS applications
///
/// This function scans standard macOS application directories and returns
/// a list of all found .app bundles. Results are cached after the first call
/// for performance (applications don't change frequently).
///
/// # Returns
/// A reference to the cached vector of AppInfo structs.
///
/// # Performance
/// Initial scan may take ~100ms depending on the number of installed apps.
/// Subsequent calls return immediately from cache.
pub fn scan_applications() -> &'static Vec<AppInfo> {
    APP_CACHE.get_or_init(|| {
        let start = Instant::now();
        let apps = scan_all_directories();
        let duration_ms = start.elapsed().as_millis();

        info!(
            app_count = apps.len(),
            duration_ms = duration_ms,
            "Scanned applications"
        );

        apps
    })
}

/// Force a fresh scan of applications (bypasses cache)
///
/// This is useful if you need to detect newly installed applications.
/// Note: This does NOT update the static cache - it just returns fresh results.
#[allow(dead_code)]
pub fn scan_applications_fresh() -> Vec<AppInfo> {
    let start = Instant::now();
    let apps = scan_all_directories();
    let duration_ms = start.elapsed().as_millis();

    info!(
        app_count = apps.len(),
        duration_ms = duration_ms,
        "Fresh scan of applications"
    );

    apps
}

/// Scan all configured directories for applications
fn scan_all_directories() -> Vec<AppInfo> {
    let mut apps = Vec::new();

    for dir in APP_DIRECTORIES {
        let expanded = shellexpand::tilde(dir);
        let path = Path::new(expanded.as_ref());

        if path.exists() {
            match scan_directory(path) {
                Ok(found) => {
                    debug!(
                        directory = %path.display(),
                        count = found.len(),
                        "Scanned directory"
                    );
                    apps.extend(found);
                }
                Err(e) => {
                    warn!(
                        directory = %path.display(),
                        error = %e,
                        "Failed to scan directory"
                    );
                }
            }
        } else {
            debug!(directory = %path.display(), "Directory does not exist, skipping");
        }
    }

    // Sort by name for consistent ordering
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // Remove duplicates (same name from different directories - prefer first)
    apps.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());

    apps
}

/// Scan a single directory for .app bundles
fn scan_directory(dir: &Path) -> Result<Vec<AppInfo>> {
    let mut apps = Vec::new();

    let entries = fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();

        // Check if it's a .app bundle
        if let Some(extension) = path.extension() {
            if extension == "app" {
                if let Some(app_info) = parse_app_bundle(&path) {
                    apps.push(app_info);
                }
            }
        }
    }

    Ok(apps)
}

/// Parse a .app bundle to extract application information
fn parse_app_bundle(path: &Path) -> Option<AppInfo> {
    // Extract app name from bundle name (strip .app extension)
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())?;

    // Try to extract bundle identifier from Info.plist
    let bundle_id = extract_bundle_id(path);

    Some(AppInfo {
        name,
        path: path.to_path_buf(),
        bundle_id,
    })
}

/// Extract CFBundleIdentifier from Info.plist
///
/// Uses /usr/libexec/PlistBuddy for reliable plist parsing.
fn extract_bundle_id(app_path: &Path) -> Option<String> {
    let plist_path = app_path.join("Contents/Info.plist");

    if !plist_path.exists() {
        return None;
    }

    // Use PlistBuddy to extract CFBundleIdentifier (reliable and fast)
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleIdentifier", plist_path.to_str()?])
        .output()
        .ok()?;

    if output.status.success() {
        let bundle_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !bundle_id.is_empty() {
            return Some(bundle_id);
        }
    }

    None
}

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
/// # Example
/// ```ignore
/// let apps = scan_applications();
/// if let Some(finder) = apps.iter().find(|a| a.name == "Finder") {
///     launch_application(finder)?;
/// }
/// ```
pub fn launch_application(app: &AppInfo) -> Result<()> {
    info!(
        app_name = %app.name,
        app_path = %app.path.display(),
        "Launching application"
    );

    Command::new("open")
        .arg("-a")
        .arg(&app.path)
        .spawn()
        .with_context(|| format!("Failed to launch application: {}", app.name))?;

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
    let apps = scan_applications();
    let name_lower = name.to_lowercase();

    let app = apps
        .iter()
        .find(|a| a.name.to_lowercase() == name_lower)
        .with_context(|| format!("Application not found: {}", name))?;

    launch_application(app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_applications_returns_apps() {
        let apps = scan_applications();

        // Should find at least some apps on any macOS system
        assert!(
            !apps.is_empty(),
            "Should find at least some applications on macOS"
        );

        // Check that Calculator exists (it's always present in /System/Applications on macOS)
        let calculator = apps.iter().find(|a| a.name == "Calculator");
        assert!(calculator.is_some(), "Calculator.app should be found");

        if let Some(calculator) = calculator {
            assert!(
                calculator.path.exists(),
                "Calculator path should exist: {:?}",
                calculator.path
            );
            assert!(calculator.bundle_id.is_some(), "Calculator should have a bundle ID");
            assert_eq!(
                calculator.bundle_id.as_deref(),
                Some("com.apple.calculator"),
                "Calculator bundle ID should be com.apple.calculator"
            );
        }
    }

    #[test]
    fn test_scan_applications_cached() {
        // First call populates cache
        let apps1 = scan_applications();

        // Second call should return same reference (cached)
        let apps2 = scan_applications();

        // Both should point to the same data
        assert_eq!(apps1.len(), apps2.len());
        assert!(std::ptr::eq(apps1, apps2), "Should return cached reference");
    }

    #[test]
    fn test_app_info_has_required_fields() {
        let apps = scan_applications();

        for app in apps.iter().take(10) {
            // Name should not be empty
            assert!(!app.name.is_empty(), "App name should not be empty");

            // Path should end with .app
            assert!(
                app.path.extension().map(|e| e == "app").unwrap_or(false),
                "App path should end with .app: {:?}",
                app.path
            );

            // Path should exist
            assert!(app.path.exists(), "App path should exist: {:?}", app.path);
        }
    }

    #[test]
    fn test_apps_sorted_alphabetically() {
        let apps = scan_applications();

        // Verify apps are sorted by lowercase name
        for window in apps.windows(2) {
            let a = &window[0];
            let b = &window[1];
            assert!(
                a.name.to_lowercase() <= b.name.to_lowercase(),
                "Apps should be sorted: {} should come before {}",
                a.name,
                b.name
            );
        }
    }

    #[test]
    fn test_extract_bundle_id_finder() {
        let finder_path = Path::new("/System/Applications/Finder.app");
        if finder_path.exists() {
            let bundle_id = extract_bundle_id(finder_path);
            assert_eq!(
                bundle_id,
                Some("com.apple.finder".to_string()),
                "Should extract Finder bundle ID"
            );
        }
    }

    #[test]
    fn test_extract_bundle_id_nonexistent() {
        let fake_path = Path::new("/nonexistent/Fake.app");
        let bundle_id = extract_bundle_id(fake_path);
        assert!(
            bundle_id.is_none(),
            "Should return None for nonexistent app"
        );
    }

    #[test]
    fn test_parse_app_bundle() {
        let finder_path = Path::new("/System/Applications/Finder.app");
        if finder_path.exists() {
            let app_info = parse_app_bundle(finder_path);
            assert!(app_info.is_some(), "Should parse Finder.app");

            let app = app_info.unwrap();
            assert_eq!(app.name, "Finder");
            assert_eq!(app.path, finder_path);
            assert!(app.bundle_id.is_some());
        }
    }

    #[test]
    fn test_no_duplicate_apps() {
        let apps = scan_applications();
        let mut names: Vec<_> = apps.iter().map(|a| a.name.to_lowercase()).collect();
        let original_len = names.len();
        names.dedup();

        assert_eq!(
            original_len,
            names.len(),
            "Should not have duplicate app names"
        );
    }

    // Note: launch_application is not tested automatically to avoid
    // actually launching apps during test runs. It can be tested manually.
}

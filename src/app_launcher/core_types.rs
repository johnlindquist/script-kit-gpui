use anyhow::{Context, Result};
use rayon::prelude::*;
use rusqlite::{params, Connection};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::Instant;
use tracing::{debug, info, info_span, trace, warn};

/// Stats for icon extraction during a scan (thread-safe)
static ICONS_EXTRACTED: AtomicUsize = AtomicUsize::new(0);
static ICONS_FROM_CACHE: AtomicUsize = AtomicUsize::new(0);
static ICONS_FROM_BUNDLE_RESOURCE: AtomicUsize = AtomicUsize::new(0);
static ICONS_FROM_ICON_SERVICES: AtomicUsize = AtomicUsize::new(0);
static ICONS_SKIPPED_ICON_SERVICES: AtomicUsize = AtomicUsize::new(0);
static EXTRACT_TIME_MS: AtomicUsize = AtomicUsize::new(0);

#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::NSString as CocoaNSString;
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};

/// Pre-decoded icon and its immutable content digest, computed once off the render path.
#[derive(Clone, Debug)]
pub struct DecodedIcon {
    image: Arc<gpui::RenderImage>,
    content_digest: [u8; 32],
}

impl DecodedIcon {
    #[expect(
        clippy::expect_used,
        reason = "frame_count bounds every immutable RenderImage frame."
    )]
    pub fn new(image: Arc<gpui::RenderImage>) -> Self {
        use sha2::Digest;
        let mut digest = sha2::Sha256::new();
        digest.update((image.frame_count() as u64).to_be_bytes());
        for frame in 0..image.frame_count() {
            let size = image.size(frame);
            let (numerator, denominator) = image.delay(frame).numer_denom_ms();
            for value in [
                size.width.0 as u64,
                size.height.0 as u64,
                numerator as u64,
                denominator as u64,
            ] {
                digest.update(value.to_be_bytes());
            }
            let bytes = image.as_bytes(frame).expect("existing icon frame");
            digest.update((bytes.len() as u64).to_be_bytes());
            digest.update(bytes);
        }
        Self {
            image,
            content_digest: digest.finalize().into(),
        }
    }

    pub fn image(&self) -> &Arc<gpui::RenderImage> {
        &self.image
    }

    pub fn into_image(self) -> Arc<gpui::RenderImage> {
        self.image
    }

    pub(crate) fn content_digest(&self) -> &[u8; 32] {
        &self.content_digest
    }
}

/// Information about an installed application
#[derive(Clone)]
pub struct AppInfo {
    /// Display name of the application (e.g., "Safari")
    pub name: String,
    /// Full path to the .app bundle (e.g., "/Applications/Safari.app")
    pub path: PathBuf,
    /// Bundle identifier from Info.plist (e.g., "com.apple.Safari")
    pub bundle_id: Option<String>,
    /// Pre-decoded icon image (32x32), ready for rendering
    /// **IMPORTANT**: This is pre-decoded to avoid PNG decoding on every render frame
    pub icon: Option<DecodedIcon>,
}

#[allow(dead_code)]
pub(crate) struct AppIconLookup {
    by_bundle_id: HashMap<String, DecodedIcon>,
    by_path: HashMap<PathBuf, DecodedIcon>,
    by_name: HashMap<String, DecodedIcon>,
}

#[allow(dead_code)]
impl AppIconLookup {
    pub(crate) fn from_apps(apps: &[AppInfo]) -> Self {
        let mut by_bundle_id = HashMap::new();
        let mut by_path = HashMap::new();
        let mut by_name = HashMap::new();

        for app in apps {
            let Some(icon) = app.icon.clone() else {
                continue;
            };
            if let Some(bundle_id) = app.bundle_id.as_ref().filter(|value| !value.is_empty()) {
                by_bundle_id
                    .entry(bundle_id.clone())
                    .or_insert_with(|| icon.clone());
            }
            by_path
                .entry(app.path.clone())
                .or_insert_with(|| icon.clone());
            by_name
                .entry(app.name.to_lowercase())
                .or_insert_with(|| icon.clone());
        }

        Self {
            by_bundle_id,
            by_path,
            by_name,
        }
    }

    pub(crate) fn icon_for_window(
        &self,
        window: &crate::window_control::WindowInfo,
    ) -> Option<DecodedIcon> {
        if let Some(bundle_id) = window.bundle_id.as_ref() {
            if let Some(icon) = self.by_bundle_id.get(bundle_id) {
                return Some(icon.clone());
            }
        }
        if let Some(path) = window.app_path.as_ref() {
            if let Some(icon) = self.by_path.get(path) {
                return Some(icon.clone());
            }
        }
        self.by_name.get(&window.app.to_lowercase()).cloned()
    }
}

impl std::fmt::Debug for AppInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppInfo")
            .field("name", &self.name)
            .field("path", &self.path)
            .field("bundle_id", &self.bundle_id)
            .field("icon", &self.icon.as_ref().map(|_| "<RenderImage>"))
            .finish()
    }
}

/// Loading state for the app cache
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLoadingState {
    /// Initial load from SQLite cache (instant, no disk scan)
    LoadingFromCache,
    /// Background directory scan in progress to find new/changed apps
    ScanningDirectories,
    /// All apps loaded and cache is up to date
    Ready,
    /// The most recent scan failed; previously valid applications remain retained.
    Failed,
}

impl AppLoadingState {
    /// Get a human-readable message for UI display
    #[allow(dead_code)]
    pub fn message(&self) -> &'static str {
        match self {
            AppLoadingState::LoadingFromCache => "Loading apps...",
            AppLoadingState::ScanningDirectories => "Scanning for new apps...",
            AppLoadingState::Ready => "Apps ready",
            AppLoadingState::Failed => "Application scan unavailable",
        }
    }
}

/// Last successful application snapshot, retained even when a later scan fails.
#[derive(Default)]
struct AppCache {
    apps: Option<Vec<AppInfo>>,
    failure: Option<String>,
}

static APP_CACHE: Mutex<AppCache> = Mutex::new(AppCache {
    apps: None,
    failure: None,
});
// Serializes native scans without holding the snapshot lock during filesystem IO.
static APP_SCAN_LOCK: Mutex<()> = Mutex::new(());

fn app_cache_snapshot(cache: &Mutex<AppCache>) -> Result<Option<Vec<AppInfo>>> {
    let cache = cache
        .lock()
        .map_err(|_| anyhow::anyhow!("Application cache lock poisoned"))?;
    if let Some(error) = &cache.failure {
        anyhow::bail!("Application scan unavailable: {error}");
    }
    Ok(cache.apps.clone())
}

fn complete_app_scan(
    cache: &Mutex<AppCache>,
    result: Result<Vec<AppInfo>>,
) -> Result<Vec<AppInfo>> {
    let mut cache = cache
        .lock()
        .map_err(|_| anyhow::anyhow!("Application cache lock poisoned"))?;
    match result {
        Ok(apps) => {
            cache.apps = Some(apps.clone());
            cache.failure = None;
            Ok(apps)
        }
        Err(error) => {
            cache.failure = Some(format!("{error:#}"));
            Err(error)
        }
    }
}

/// Current loading state (thread-safe, updated during scan)
static APP_LOADING_STATE: LazyLock<Mutex<AppLoadingState>> =
    LazyLock::new(|| Mutex::new(AppLoadingState::LoadingFromCache));

/// Database connection for apps cache
static APPS_DB: OnceLock<Arc<Mutex<Connection>>> = OnceLock::new();

/// Directories to scan for .app bundles
const APP_DIRECTORIES: &[&str] = &[
    // Standard macOS app locations
    "/Applications",
    "/System/Applications",
    "/System/Applications/Utilities",
    "/Applications/Utilities",
    // Finder lives directly in CoreServices rather than the Applications subfolder.
    "/System/Library/CoreServices",
    // System utilities (Keychain Access, Screen Sharing, etc.)
    "/System/Library/CoreServices/Applications",
    // User-specific apps
    "~/Applications",
    // Chrome installed web apps (PWAs)
    "~/Applications/Chrome Apps.localized",
    // Edge installed web apps (PWAs)
    "~/Applications/Edge Apps.localized",
    // Arc browser installed web apps
    "~/Applications/Arc Apps",
    // Setapp subscription apps (if installed)
    "/Applications/Setapp",
];

// ============================================================================
// SQLite Database Functions
// ============================================================================

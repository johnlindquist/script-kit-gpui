#![allow(dead_code)]
//! macOS "Open With" helpers for clipboard temp files.

use std::path::{Path, PathBuf};

/// Application info returned by Launch Services.
#[derive(Debug, Clone)]
pub struct AppInfo {
    pub name: String,
    pub bundle_id: Option<String>,
    pub app_path: PathBuf,
}

#[cfg(target_os = "macos")]
mod macos {
    use super::AppInfo;
    use core_foundation::array::CFArray;
    use core_foundation::array::CFArrayRef;
    use core_foundation::base::TCFType;
    use core_foundation::bundle::CFBundle;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::error::CFErrorRef;
    use core_foundation::string::CFString;
    use core_foundation::url::CFURLRef;
    use core_foundation::url::CFURL;
    use std::collections::HashSet;
    use std::path::Path;

    const LS_ROLES_ALL: u32 = 0xFFFFFFFF;

    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        fn LSCopyApplicationURLsForURL(
            in_url: CFURLRef,
            in_role_mask: u32,
            out_error: *mut CFErrorRef,
        ) -> CFArrayRef;
    }

    pub(super) fn get_apps_for_file(path: &Path) -> Vec<AppInfo> {
        let Some(file_url) = CFURL::from_path(path, false) else {
            return Vec::new();
        };

        // SAFETY: file_url is a valid CFURL created above. LSCopyApplicationURLsForURL
        // returns a CFArray that we wrap with create-rule (transfers ownership).
        // The null error pointer opts out of error reporting.
        unsafe {
            let array_ref = LSCopyApplicationURLsForURL(
                file_url.as_concrete_TypeRef(),
                LS_ROLES_ALL,
                std::ptr::null_mut(),
            );
            if array_ref.is_null() {
                return Vec::new();
            }

            let urls: CFArray<CFURL> = TCFType::wrap_under_create_rule(array_ref);
            let mut seen = HashSet::new();
            let mut apps = Vec::new();

            for app_url in urls.iter() {
                let Some(app_path) = app_url.to_path() else {
                    continue;
                };

                if !seen.insert(app_path.clone()) {
                    continue;
                }

                let (name, bundle_id) = bundle_metadata(&app_path);
                apps.push(AppInfo {
                    name,
                    bundle_id,
                    app_path,
                });
            }

            apps
        }
    }

    pub(super) fn open_file_with_app(file_path: &Path, app_path: &Path) -> Result<(), String> {
        std::process::Command::new("open")
            .arg("-a")
            .arg(app_path)
            .arg(file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file with app: {}", e))?;
        Ok(())
    }

    fn bundle_metadata(app_path: &Path) -> (String, Option<String>) {
        let fallback_name = app_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Application")
            .to_string();

        let Some(bundle_url) = CFURL::from_path(app_path, true) else {
            return (fallback_name, None);
        };
        let Some(bundle) = CFBundle::new(bundle_url) else {
            return (fallback_name, None);
        };

        let info = bundle.info_dictionary();
        let name = bundle_string_value(&info, "CFBundleDisplayName")
            .or_else(|| bundle_string_value(&info, "CFBundleName"))
            .unwrap_or(fallback_name);
        let bundle_id = bundle_string_value(&info, "CFBundleIdentifier");

        (name, bundle_id)
    }

    fn bundle_string_value(
        info: &CFDictionary<CFString, core_foundation::base::CFType>,
        key: &str,
    ) -> Option<String> {
        let key = CFString::new(key);
        let value = info.find(key)?;
        value
            .downcast::<CFString>()
            .map(|cf_str| cf_str.to_string())
    }
}

#[cfg(target_os = "macos")]
pub fn get_apps_for_file(path: &Path) -> Vec<AppInfo> {
    if let Err(error) =
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)
    {
        tracing::warn!(%error, "Launch Services lookup refused");
        return Vec::new();
    }
    macos::get_apps_for_file(path)
}

#[cfg(target_os = "macos")]
pub fn open_file_with_app(file_path: &Path, app_path: &Path) -> Result<(), String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal)
        .map_err(|error| error.to_string())?;
    macos::open_file_with_app(file_path, app_path)
}

#[cfg(not(target_os = "macos"))]
pub fn get_apps_for_file(_path: &Path) -> Vec<AppInfo> {
    Vec::new()
}

#[cfg(not(target_os = "macos"))]
pub fn open_file_with_app(_file_path: &Path, _app_path: &Path) -> Result<(), String> {
    Err("Open With is only supported on macOS".to_string())
}

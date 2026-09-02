use std::path::PathBuf;

pub fn host_app_bundle_url() -> anyhow::Result<PathBuf> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    #[cfg(target_os = "macos")]
    {
        if let Some(path) = main_bundle_path() {
            if path.extension().and_then(|ext| ext.to_str()) == Some("app") && path.is_dir() {
                return Ok(path);
            }
        }
    }

    let executable = std::env::current_exe()?;
    for ancestor in executable.ancestors() {
        if ancestor.extension().and_then(|ext| ext.to_str()) == Some("app") && ancestor.is_dir() {
            return Ok(ancestor.to_path_buf());
        }
    }

    anyhow::bail!(
        "Script Kit host app bundle was not found for {}",
        executable.display()
    );
}

#[cfg(target_os = "macos")]
fn main_bundle_path() -> Option<PathBuf> {
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::CStr;

    unsafe {
        let bundle: id = msg_send![class!(NSBundle), mainBundle];
        if bundle == nil {
            return None;
        }
        let bundle_url: id = msg_send![bundle, bundleURL];
        if bundle_url == nil {
            return None;
        }
        let path: id = msg_send![bundle_url, path];
        if path == nil {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![path, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(PathBuf::from(
            CStr::from_ptr(utf8).to_string_lossy().into_owned(),
        ))
    }
}

pub fn host_app_display_name() -> String {
    host_app_bundle_url()
        .ok()
        .and_then(|path| {
            path.file_stem()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Script Kit".to_string())
}

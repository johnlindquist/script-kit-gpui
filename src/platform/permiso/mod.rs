pub mod drag_source;
pub mod host_app;
pub mod locator;
pub mod overlay_window;
pub mod panel;

pub use panel::PermisoPanel;

pub struct PermisoAssistant;
pub struct PermisoHandle {
    controller: Option<overlay_window::OverlayController>,
}

static ACTIVE_PERMISO_HANDLE: std::sync::OnceLock<parking_lot::Mutex<Option<PermisoHandle>>> =
    std::sync::OnceLock::new();

fn active_permiso_handle() -> &'static parking_lot::Mutex<Option<PermisoHandle>> {
    ACTIVE_PERMISO_HANDLE.get_or_init(|| parking_lot::Mutex::new(None))
}

impl Drop for PermisoHandle {
    fn drop(&mut self) {
        if let Some(controller) = self.controller.as_mut() {
            controller.dismiss();
        }
    }
}

impl PermisoAssistant {
    pub fn present(panel: PermisoPanel) -> anyhow::Result<PermisoHandle> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeVisibility)?;
        present_settings_url(panel)?;
        let controller = overlay_window::OverlayController::present(panel)?;
        Ok(PermisoHandle {
            controller: Some(controller),
        })
    }

    pub fn present_retained(panel: PermisoPanel) -> anyhow::Result<()> {
        let handle = Self::present(panel)?;
        *active_permiso_handle().lock() = Some(handle);
        Ok(())
    }

    pub fn dismiss_active() {
        *active_permiso_handle().lock() = None;
    }
}

#[cfg(target_os = "macos")]
pub fn present_settings_url(panel: PermisoPanel) -> anyhow::Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal)?;
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::CString;

    let url = CString::new(panel.settings_url())?;

    unsafe {
        let ns_string: id = msg_send![class!(NSString), stringWithUTF8String: url.as_ptr()];
        if ns_string == nil {
            anyhow::bail!("failed to create settings URL string");
        }

        let ns_url: id = msg_send![class!(NSURL), URLWithString: ns_string];
        if ns_url == nil {
            anyhow::bail!("failed to create settings URL");
        }

        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            anyhow::bail!("failed to access NSWorkspace");
        }

        let opened: bool = msg_send![workspace, openURL: ns_url];
        if !opened {
            anyhow::bail!("failed to open {} settings", panel.display_name());
        }
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn present_settings_url(_panel: PermisoPanel) -> anyhow::Result<()> {
    anyhow::bail!("Permission assistant is only available on macOS");
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveAppIdentity {
    pub name: String,
    pub bundle_id: Option<String>,
    pub process_id: Option<i32>,
}

impl ActiveAppIdentity {
    pub fn unknown() -> Self {
        Self {
            name: "Unknown".to_string(),
            bundle_id: None,
            process_id: None,
        }
    }
}

#[cfg(target_os = "macos")]
pub fn current_frontmost_app_identity() -> ActiveAppIdentity {
    if !super::super::native_effect_allowed(crate::runtime_policy::ExternalEffect::SystemDiscovery)
    {
        return ActiveAppIdentity::unknown();
    }
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let Some(workspace_class) = Class::get("NSWorkspace") else {
            return ActiveAppIdentity::unknown();
        };
        let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
        if workspace.is_null() {
            return ActiveAppIdentity::unknown();
        }

        let app: *mut Object = msg_send![workspace, menuBarOwningApplication];
        let app = if app.is_null() {
            msg_send![workspace, frontmostApplication]
        } else {
            app
        };
        if app.is_null() {
            return ActiveAppIdentity::unknown();
        }

        let bundle_id = nsstring_to_string(msg_send![app, bundleIdentifier]);
        let name = nsstring_to_string(msg_send![app, localizedName])
            .or_else(|| bundle_id.clone())
            .unwrap_or_else(|| "Unknown".to_string());
        let pid: i32 = msg_send![app, processIdentifier];

        ActiveAppIdentity {
            name,
            bundle_id,
            process_id: (pid > 0).then_some(pid),
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn current_frontmost_app_identity() -> ActiveAppIdentity {
    ActiveAppIdentity::unknown()
}

#[cfg(target_os = "macos")]
fn nsstring_to_string(obj: *mut objc::runtime::Object) -> Option<String> {
    if obj.is_null() {
        return None;
    }

    unsafe {
        use objc::{msg_send, sel, sel_impl};
        let utf8: *const i8 = msg_send![obj, UTF8String];
        if utf8.is_null() {
            return None;
        }
        std::ffi::CStr::from_ptr(utf8)
            .to_str()
            .ok()
            .map(str::to_string)
    }
}

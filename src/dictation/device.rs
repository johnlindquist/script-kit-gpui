use crate::dictation::types::{DictationDeviceId, DictationDeviceInfo, DictationDeviceTransport};
#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::Result;

// ---------------------------------------------------------------------------
// Device selection types
// ---------------------------------------------------------------------------

/// Action produced by the microphone picker UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationDeviceSelectionAction {
    UseSystemDefault,
    UseDevice(DictationDeviceId),
}

pub const DICTATION_SYSTEM_DEFAULT_DEVICE_VALUE: &str = "__system_default__";

/// A single row in the microphone picker list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationDeviceMenuItem {
    pub title: String,
    pub subtitle: String,
    pub action: DictationDeviceSelectionAction,
    pub is_selected: bool,
}

pub fn device_selection_value(action: &DictationDeviceSelectionAction) -> String {
    match action {
        DictationDeviceSelectionAction::UseSystemDefault => {
            DICTATION_SYSTEM_DEFAULT_DEVICE_VALUE.to_string()
        }
        DictationDeviceSelectionAction::UseDevice(device_id) => device_id.0.clone(),
    }
}

pub fn device_selection_action_from_value(value: &str) -> DictationDeviceSelectionAction {
    if value == DICTATION_SYSTEM_DEFAULT_DEVICE_VALUE {
        DictationDeviceSelectionAction::UseSystemDefault
    } else {
        DictationDeviceSelectionAction::UseDevice(DictationDeviceId(value.to_string()))
    }
}

pub fn microphone_display_label(title: &str) -> String {
    let title = title
        .replace(" \u{00b7} default", "")
        .replace(" (current)", "");
    let title = title.trim();
    if title.eq_ignore_ascii_case("system default") {
        return "System Default".to_string();
    }

    if title.is_empty() {
        "Microphone".to_string()
    } else {
        title.to_string()
    }
}

// ---------------------------------------------------------------------------
// Pure selection helpers (no I/O — easy to test)
// ---------------------------------------------------------------------------

/// Outcome of microphone resolution, including whether a fallback was used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceResolution {
    pub device: DictationDeviceInfo,
    /// When `true`, the user's saved preference was not found and we fell back
    /// to a heuristic choice.  Callers should surface this to the user.
    pub fell_back: bool,
}

/// Resolve a microphone from the device list using a saved preference.
///
/// Priority order:
/// 1. Exact match on `selected_device_id` (user's saved preference).
/// 2. System default device (`is_default == true`).
/// 3. Built-in microphone (transport == BuiltIn).
/// 4. USB microphone (transport == Usb).
/// 5. First non-virtual device.
/// 6. Any device at all.
///
/// Sets `fell_back = true` when the saved preference existed but wasn't
/// found in the current device list.
pub fn resolve_selected_input_device(
    devices: &[DictationDeviceInfo],
    selected_device_id: Option<&str>,
) -> Option<DeviceResolution> {
    if devices.is_empty() {
        return None;
    }

    // 1. Exact match on saved preference.
    if let Some(saved) = selected_device_id {
        if let Some(device) = devices.iter().find(|d| d.id.0 == saved) {
            return Some(DeviceResolution {
                device: device.clone(),
                fell_back: false,
            });
        }
        // Saved device disappeared — fall through with fell_back flag.
        tracing::warn!(
            category = "DICTATION",
            saved_id = saved,
            "Saved microphone not found in device list, falling back"
        );
        return Some(DeviceResolution {
            device: pick_best_fallback(devices),
            fell_back: true,
        });
    }

    // No saved preference — use heuristic without flagging fallback.
    Some(DeviceResolution {
        device: pick_best_fallback(devices),
        fell_back: false,
    })
}

/// Pick the best microphone from the device list using a ranked heuristic.
fn pick_best_fallback(devices: &[DictationDeviceInfo]) -> DictationDeviceInfo {
    // System default first.
    if let Some(d) = devices.iter().find(|d| d.is_default) {
        return d.clone();
    }
    // Built-in mic.
    if let Some(d) = devices
        .iter()
        .find(|d| d.transport == DictationDeviceTransport::BuiltIn)
    {
        return d.clone();
    }
    // USB mic (external but physical).
    if let Some(d) = devices
        .iter()
        .find(|d| d.transport == DictationDeviceTransport::Usb)
    {
        return d.clone();
    }
    // Any non-virtual device.
    if let Some(d) = devices
        .iter()
        .find(|d| d.transport != DictationDeviceTransport::Virtual)
    {
        return d.clone();
    }
    // Last resort: first device.
    devices[0].clone()
}

/// Build the full menu item list for the microphone picker.
///
/// Always includes a "System Default" row at the top, followed by one row per
/// enumerated device.  Exactly one row will have `is_selected == true`.
pub fn build_device_menu_items(
    devices: &[DictationDeviceInfo],
    selected_device_id: Option<&str>,
) -> Vec<DictationDeviceMenuItem> {
    let saved_exists = selected_device_id
        .map(|saved| devices.iter().any(|d| d.id.0 == saved))
        .unwrap_or(false);

    let mut items = Vec::with_capacity(devices.len() + 1);

    items.push(DictationDeviceMenuItem {
        title: "System Default".to_string(),
        subtitle: devices
            .iter()
            .find(|d| d.is_default)
            .map(|d| format!("Use macOS default microphone ({})", d.name))
            .unwrap_or_else(|| "Use macOS default microphone".to_string()),
        action: DictationDeviceSelectionAction::UseSystemDefault,
        is_selected: selected_device_id.is_none() || !saved_exists,
    });

    for device in devices {
        items.push(DictationDeviceMenuItem {
            title: if device.is_default {
                format!("{} \u{00b7} default", device.name)
            } else {
                device.name.clone()
            },
            subtitle: if device.is_default {
                "Currently the macOS default input".to_string()
            } else {
                "Use this microphone for dictation".to_string()
            },
            action: DictationDeviceSelectionAction::UseDevice(device.id.clone()),
            is_selected: selected_device_id == Some(device.id.0.as_str()),
        });
    }

    items
}

// ---------------------------------------------------------------------------
// I/O wrappers (thin shells over the pure helpers + persistence)
// ---------------------------------------------------------------------------

/// Enumerate input devices and build a ready-to-render menu item list.
pub fn list_input_device_menu_items(
    selected_device_id: Option<&str>,
) -> Result<Vec<DictationDeviceMenuItem>> {
    let devices = list_input_devices().context("failed to enumerate microphones")?;
    Ok(build_device_menu_items(&devices, selected_device_id))
}

pub fn microphone_permission_status() -> crate::dictation::DictationMicrophonePermissionStatus {
    // Read-only AVFoundation preflight: authorizationStatusForMediaType only,
    // never asks macOS for a permission prompt from setup.
    match crate::platform::permiso_detect::microphone_authorized() {
        crate::platform::permiso_detect::PermissionStatus::Authorized => {
            crate::dictation::DictationMicrophonePermissionStatus::Granted
        }
        crate::platform::permiso_detect::PermissionStatus::Denied => {
            crate::dictation::DictationMicrophonePermissionStatus::Denied
        }
        crate::platform::permiso_detect::PermissionStatus::NotDetermined => {
            crate::dictation::DictationMicrophonePermissionStatus::NotDetermined
        }
        crate::platform::permiso_detect::PermissionStatus::Unknown => {
            crate::dictation::DictationMicrophonePermissionStatus::Unknown
        }
    }
}

#[cfg(target_os = "macos")]
pub fn request_microphone_permission() -> crate::dictation::DictationMicrophonePermissionStatus {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use std::sync::mpsc;
    use std::time::Duration;

    if !matches!(
        microphone_permission_status(),
        crate::dictation::DictationMicrophonePermissionStatus::NotDetermined
    ) {
        return microphone_permission_status();
    }

    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}

    let (tx, rx) = mpsc::channel();
    let block = block::ConcreteBlock::new(move |granted: BOOL| {
        let _ = tx.send(granted == YES);
    });
    let block = block.copy();

    unsafe {
        let media_type = NSString::alloc(nil).init_str("soun");
        if media_type == nil {
            return crate::dictation::DictationMicrophonePermissionStatus::Unknown;
        }

        let _: () = msg_send![
            class!(AVCaptureDevice),
            requestAccessForMediaType: media_type as id
            completionHandler: &*block
        ];
    }

    match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(true) => crate::dictation::DictationMicrophonePermissionStatus::Granted,
        Ok(false) => crate::dictation::DictationMicrophonePermissionStatus::Denied,
        Err(_) => crate::dictation::DictationMicrophonePermissionStatus::Unknown,
    }
}

#[cfg(target_os = "macos")]
pub fn request_microphone_permission_nonblocking(
    on_result: impl FnOnce(crate::dictation::DictationMicrophonePermissionStatus) + Send + 'static,
) {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use std::sync::{Arc, Mutex};

    if !matches!(
        microphone_permission_status(),
        crate::dictation::DictationMicrophonePermissionStatus::NotDetermined
    ) {
        on_result(microphone_permission_status());
        return;
    }

    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}

    let callback = Arc::new(Mutex::new(Some(Box::new(on_result)
        as Box<
            dyn FnOnce(crate::dictation::DictationMicrophonePermissionStatus) + Send,
        >)));
    let callback_for_block = callback.clone();
    let block = block::ConcreteBlock::new(move |granted: BOOL| {
        if let Ok(mut callback) = callback_for_block.lock() {
            if let Some(callback) = callback.take() {
                callback(if granted == YES {
                    crate::dictation::DictationMicrophonePermissionStatus::Granted
                } else {
                    crate::dictation::DictationMicrophonePermissionStatus::Denied
                });
            }
        }
    });
    let block = block.copy();

    unsafe {
        let media_type = NSString::alloc(nil).init_str("soun");
        if media_type == nil {
            if let Ok(mut callback) = callback.lock() {
                if let Some(callback) = callback.take() {
                    callback(crate::dictation::DictationMicrophonePermissionStatus::Unknown);
                }
            }
            return;
        }

        let _: () = msg_send![
            class!(AVCaptureDevice),
            requestAccessForMediaType: media_type as id
            completionHandler: &*block
        ];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn request_microphone_permission() -> crate::dictation::DictationMicrophonePermissionStatus {
    crate::dictation::DictationMicrophonePermissionStatus::Unknown
}

#[cfg(not(target_os = "macos"))]
pub fn request_microphone_permission_nonblocking(
    on_result: impl FnOnce(crate::dictation::DictationMicrophonePermissionStatus) + Send + 'static,
) {
    on_result(crate::dictation::DictationMicrophonePermissionStatus::Unknown);
}

/// Persist a picker selection to config-backed preferences.
pub fn apply_device_selection(action: &DictationDeviceSelectionAction) -> Result<()> {
    match action {
        DictationDeviceSelectionAction::UseSystemDefault => save_dictation_device_id(None),
        DictationDeviceSelectionAction::UseDevice(device_id) => {
            save_dictation_device_id(Some(device_id.0.as_str()))
        }
    }
}

/// Persist the selected microphone device ID to config-backed preferences.
///
/// Pass `None` to clear the preference and revert to the system default.
pub fn save_dictation_device_id(device_id: Option<&str>) -> Result<()> {
    let mut prefs = crate::config::load_user_preferences();
    prefs.dictation.selected_device_id = device_id.map(str::to_owned);
    crate::config::save_user_preferences(&prefs)?;
    tracing::info!(
        category = "DICTATION",
        device_id = ?device_id,
        "Saved microphone preference"
    );
    crate::dictation::runtime::notify_dictation_device_preference_changed();
    Ok(())
}

/// Persist the selected dictation model catalog ID.
///
/// Pass `None` to clear the preference and revert to the default model.
pub fn save_dictation_model(model: Option<&str>) -> Result<()> {
    let mut prefs = crate::config::load_user_preferences();
    prefs.dictation.model = model.map(str::to_owned);
    crate::config::save_user_preferences(&prefs)?;
    tracing::info!(
        category = "DICTATION",
        model = ?model,
        "Saved dictation model preference"
    );
    crate::dictation::runtime::notify_dictation_model_preference_changed();
    Ok(())
}

/// Persist the sticky dictation destination (`dictation.lastTarget`).
///
/// Called when the user picks a destination via an overlay verb chip so
/// "sticky" target mode reuses it for the next session. Skips the config
/// write when the stored value already matches.
pub fn save_dictation_last_target(target: crate::dictation::DictationTarget) -> Result<()> {
    let label = target.sticky_label();
    let mut prefs = crate::config::load_user_preferences();
    if prefs.dictation.last_target.as_deref() == Some(label) {
        return Ok(());
    }
    prefs.dictation.last_target = Some(label.to_string());
    crate::config::save_user_preferences(&prefs)?;
    tracing::info!(
        category = "DICTATION",
        target = %label,
        "Saved sticky dictation destination"
    );
    Ok(())
}

/// Persist the dictation language hint.
///
/// Pass `None` to clear the language preference.
pub fn save_dictation_language(language: Option<&str>) -> Result<()> {
    let mut prefs = crate::config::load_user_preferences();
    prefs.dictation.language = language.map(str::to_owned);
    crate::config::save_user_preferences(&prefs)?;
    tracing::info!(
        category = "DICTATION",
        language = ?language,
        "Saved dictation language preference"
    );
    crate::dictation::runtime::notify_dictation_model_preference_changed();
    Ok(())
}

#[cfg(target_os = "macos")]
use objc::runtime::{Object, BOOL, YES};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};

#[cfg(target_os = "macos")]
pub fn list_input_devices() -> Result<Vec<DictationDeviceInfo>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Device)?;
    unsafe {
        // SAFETY: AVFoundation device enumeration is performed via Objective-C messaging
        // with selectors documented for AVCaptureDevice. All returned objects are treated
        // as borrowed/autoreleased and not retained by Rust.
        let default_device: *mut Object =
            msg_send![class!(AVCaptureDevice), defaultDeviceWithMediaType: av_media_type_audio()];
        let default_id = nsstring_to_string(if default_device.is_null() {
            std::ptr::null_mut()
        } else {
            let value: *mut Object = msg_send![default_device, uniqueID];
            value
        });

        let devices: *mut Object =
            msg_send![class!(AVCaptureDevice), devicesWithMediaType: av_media_type_audio()];
        if devices.is_null() {
            return Ok(Vec::new());
        }

        let count: usize = msg_send![devices, count];
        let mut items = Vec::with_capacity(count);

        for index in 0..count {
            let device: *mut Object = msg_send![devices, objectAtIndex: index];
            if device.is_null() {
                continue;
            }

            let id_obj: *mut Object = msg_send![device, uniqueID];
            let name_obj: *mut Object = msg_send![device, localizedName];

            let id = match nsstring_to_string(id_obj) {
                Some(value) => value,
                None => continue,
            };

            let name = nsstring_to_string(name_obj)
                .with_context(|| format!("missing localizedName for audio input device {id}"))?;

            let transport = classify_device_transport(device);

            items.push(DictationDeviceInfo {
                is_default: default_id.as_deref() == Some(id.as_str()),
                id: DictationDeviceId(id),
                name,
                transport,
            });
        }

        Ok(items)
    }
}

/// Classify an AVCaptureDevice's transport type into our simplified enum.
///
/// Uses the `transportType` property (CMIOObjectPropertyScope) to distinguish
/// built-in mics from USB, Bluetooth, and virtual/aggregate devices.
#[cfg(target_os = "macos")]
fn classify_device_transport(device: *mut Object) -> DictationDeviceTransport {
    // SAFETY: device is a valid AVCaptureDevice from the enumeration above.
    // transportType returns an i32 (FourCharCode) that we match against known
    // CoreMedia transport type constants.
    let transport_type: i32 = unsafe { msg_send![device, transportType] };

    // CoreMedia FourCharCode constants (from CMIOHardwareDevice.h):
    // 'bltn' = 0x626C746E = built-in
    // 'usb ' = 0x75736220 = USB
    // 'blue' = 0x626C7565 = Bluetooth
    // 'virt' = 0x76697274 = virtual
    const BUILT_IN: i32 = 0x626C_746E_u32 as i32; // 'bltn'
    const USB: i32 = 0x7573_6220_u32 as i32; // 'usb '
    const BLUETOOTH: i32 = 0x626C_7565_u32 as i32; // 'blue'
    const VIRTUAL: i32 = 0x7669_7274_u32 as i32; // 'virt'

    match transport_type {
        BUILT_IN => DictationDeviceTransport::BuiltIn,
        USB => DictationDeviceTransport::Usb,
        BLUETOOTH => DictationDeviceTransport::Bluetooth,
        VIRTUAL => DictationDeviceTransport::Virtual,
        _ => DictationDeviceTransport::Unknown,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn list_input_devices() -> Result<Vec<DictationDeviceInfo>> {
    Ok(Vec::new())
}

#[cfg(target_os = "macos")]
pub fn default_input_device() -> Result<Option<DictationDeviceInfo>> {
    let devices = list_input_devices()?;
    Ok(resolve_selected_input_device(&devices, None).map(|r| r.device))
}

#[cfg(not(target_os = "macos"))]
pub fn default_input_device() -> Result<Option<DictationDeviceInfo>> {
    Ok(None)
}

#[cfg(target_os = "macos")]
fn av_media_type_audio() -> *mut Object {
    unsafe {
        // SAFETY: The UTF-8 string literal is NUL-terminated and valid for NSString construction.
        let media_type: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: c"soun".as_ptr()];
        media_type
    }
}

#[cfg(target_os = "macos")]
unsafe fn nsstring_to_string(value: *mut Object) -> Option<String> {
    if value.is_null() {
        return None;
    }

    // SAFETY: `value` is expected to be an NSString-compatible object and UTF8String
    // returns a borrowed NUL-terminated buffer for the lifetime of the NSString.
    let utf8: *const i8 = msg_send![value, UTF8String];
    if utf8.is_null() {
        return None;
    }

    Some(
        std::ffi::CStr::from_ptr(utf8)
            .to_string_lossy()
            .into_owned(),
    )
}

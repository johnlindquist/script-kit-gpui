use anyhow::{bail, Context, Result};
use core_foundation::array::CFArray;
use core_foundation::base::{CFTypeRef as CoreFoundationTypeRef, TCFType};
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::string::CFString;
use macos_accessibility_client::accessibility;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use tracing::{debug, info, instrument, warn};

use super::ax::{
    batch_read_window_attributes, get_ax_attribute, get_window_bool_attribute, get_window_position,
    get_window_size, get_window_string_attribute,
};
use super::cache::CachedAxRef;
use super::capabilities::collect_ax_capabilities;
use super::cf::{cf_hash, cf_release, cf_retain};
use super::ffi::{
    AXUIElementCreateApplication, AXUIElementRef, CFArrayGetCount, CFArrayGetValueAtIndex,
    CFArrayRef, CFEqual,
};
use super::observation::{correlate, AxCorrelationRow, CgCorrelationRow};
use super::registry::{SearchScope, StagedWindow};
use super::types::{Bounds, NativeIdConfidence, SearchVisibility, WindowCapabilities, WindowInfo};

#[derive(Clone)]
struct RunningAppMetadata {
    app: String,
    bundle_id: Option<String>,
    app_path: Option<PathBuf>,
    app_order: usize,
    is_frontmost_app: bool,
}

#[derive(Clone)]
struct CoreGraphicsWindowInfo {
    native_window_id: u32,
    pid: i32,
    title: String,
    bounds: Bounds,
    is_on_current_space: bool,
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {
    // We'll use objc crate for AppKit access instead of direct FFI
}

/// Check if accessibility permissions are granted.
///
/// Window control operations require the application to have accessibility
/// permissions granted by the user.
///
/// # Returns
/// `true` if permission is granted, `false` otherwise.
#[instrument]
pub fn has_accessibility_permission() -> bool {
    let result = accessibility::application_is_trusted();
    debug!(granted = result, "Checked accessibility permission");
    result
}

/// Request accessibility permissions (opens System Preferences).
///
/// # Returns
/// `true` if permission is granted after the request, `false` otherwise.
#[instrument]
pub fn request_accessibility_permission() -> bool {
    info!("Requesting accessibility permission for window control");
    accessibility::application_is_trusted_with_prompt()
}

/// List all visible windows across all applications.
///
/// Returns a vector of `WindowInfo` structs containing window metadata.
/// Windows are filtered to only include visible, standard windows.
///
/// # Returns
/// A vector of window information structs.
///
/// # Errors
/// Returns error if accessibility permission is not granted.
///
#[instrument]
pub fn list_windows() -> Result<Vec<WindowInfo>> {
    if super::test_support::is_active() {
        super::registry::refresh_from_test_provider()?;
    } else {
        if !has_accessibility_permission() {
            bail!("Accessibility permission required for window control");
        }
        super::registry::refresh_from_system()?;
    }
    Ok(super::registry::window_infos(SearchScope::Ordinary))
}

/// Enumerate live windows into staged registry rows.
///
/// Preserves the historical enumeration exactly: regular-activation apps,
/// titled AX windows at least 50×50, `(pid << 16) | index` base IDs, then
/// CoreGraphics-only rows for other-Space windows with synthetic base IDs.
pub(super) fn collect_staged_system_windows() -> Result<Vec<StagedWindow>> {
    let mut windows = Vec::new();
    // Correlation facts aligned index-for-index with the AX rows in `windows`.
    let mut ax_facts: Vec<AxCorrelationRow> = Vec::new();
    let mut running_apps_by_pid = HashMap::<i32, RunningAppMetadata>::new();

    // Get list of running applications using objc
    // SAFETY: All objc msg_send! calls target well-known AppKit/Foundation classes
    // (NSWorkspace, NSRunningApplication). Pointers are null-checked before use.
    // AX element refs are properly retained/released via cf_retain/cf_release.
    unsafe {
        use objc::runtime::{Class, Object};
        use objc::{msg_send, sel, sel_impl};

        let workspace_class = Class::get("NSWorkspace").context("Failed to get NSWorkspace")?;
        let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
        let running_apps: *mut Object = msg_send![workspace, runningApplications];
        let app_count: usize = msg_send![running_apps, count];
        let frontmost_app: *mut Object = msg_send![workspace, frontmostApplication];
        let frontmost_pid: i32 = if frontmost_app.is_null() {
            -1
        } else {
            msg_send![frontmost_app, processIdentifier]
        };

        for i in 0..app_count {
            let app: *mut Object = msg_send![running_apps, objectAtIndex: i];

            // Check activation policy (skip background apps)
            let activation_policy: i64 = msg_send![app, activationPolicy];
            if activation_policy != 0 {
                // 0 = NSApplicationActivationPolicyRegular
                continue;
            }

            // Get app name
            let app_name: *mut Object = msg_send![app, localizedName];
            let app_name_str = if !app_name.is_null() {
                let utf8: *const i8 = msg_send![app_name, UTF8String];
                if !utf8.is_null() {
                    std::ffi::CStr::from_ptr(utf8)
                        .to_str()
                        .unwrap_or("Unknown")
                        .to_string()
                } else {
                    "Unknown".to_string()
                }
            } else {
                "Unknown".to_string()
            };

            // Get PID
            let pid: i32 = msg_send![app, processIdentifier];
            let bundle_id = nsstring_to_string(msg_send![app, bundleIdentifier]);
            let bundle_url: *mut Object = msg_send![app, bundleURL];
            let app_path = nsurl_to_pathbuf(bundle_url);
            let is_frontmost_app = pid == frontmost_pid;
            running_apps_by_pid.insert(
                pid,
                RunningAppMetadata {
                    app: app_name_str.clone(),
                    bundle_id: bundle_id.clone(),
                    app_path: app_path.clone(),
                    app_order: i,
                    is_frontmost_app,
                },
            );

            // Create AXUIElement for this application
            let ax_app = AXUIElementCreateApplication(pid);
            if ax_app.is_null() {
                continue;
            }

            let focused_window = get_ax_attribute(ax_app, "AXFocusedWindow").ok();
            let main_window = get_ax_attribute(ax_app, "AXMainWindow").ok();

            // Get windows for this app
            if let Ok(windows_value) = get_ax_attribute(ax_app, "AXWindows") {
                let window_count = CFArrayGetCount(windows_value as CFArrayRef);

                for j in 0..window_count {
                    // CFArrayGetValueAtIndex returns a borrowed reference - we must retain
                    // if we want to keep it beyond the array's lifetime
                    let ax_window = CFArrayGetValueAtIndex(windows_value as CFArrayRef, j);
                    let window_ref_for_reads = ax_window as AXUIElementRef;

                    // Batch-read title/role/subrole/minimized; fall back to
                    // individual reads when the batch API misbehaves.
                    let (title, role, subrole, batch_minimized) =
                        match batch_read_window_attributes(window_ref_for_reads) {
                            Some(batch) => (
                                batch.title.unwrap_or_default(),
                                batch.role,
                                batch.subrole,
                                batch.minimized,
                            ),
                            None => (
                                get_window_string_attribute(window_ref_for_reads, "AXTitle")
                                    .unwrap_or_default(),
                                get_window_string_attribute(window_ref_for_reads, "AXRole"),
                                get_window_string_attribute(window_ref_for_reads, "AXSubrole"),
                                None,
                            ),
                        };

                    // Titleless windows: keep dialogs/sheets as internal-only
                    // observations; skip other titleless utility rows (the
                    // historical behavior for ordinary listings).
                    let titleless_dialog_like = title.is_empty()
                        && matches!(role.as_deref(), Some("AXDialog") | Some("AXSheet"));
                    if title.is_empty() && !titleless_dialog_like {
                        continue;
                    }

                    // Get window position and size
                    let (x, y) = get_window_position(ax_window as AXUIElementRef).unwrap_or((0, 0));
                    let (width, height) =
                        get_window_size(ax_window as AXUIElementRef).unwrap_or((0, 0));

                    // Skip very small windows (likely invisible or popups),
                    // except internal dialog-likes which stay observable.
                    if (width < 50 || height < 50) && !titleless_dialog_like {
                        continue;
                    }

                    // Historical base ID: (pid << 16) | window_index. The
                    // registry allocates the final non-rebinding legacy ID.
                    let base_legacy_id = ((pid as u32) << 16) | (j as u32);
                    let window_ref = ax_window as AXUIElementRef;
                    let is_focused =
                        focused_window.is_some_and(|focused| CFEqual(window_ref as _, focused));
                    let is_main = main_window.is_some_and(|main| CFEqual(window_ref as _, main));
                    let is_minimized = batch_minimized.unwrap_or_else(|| {
                        get_window_bool_attribute(window_ref, "AXMinimized").unwrap_or(false)
                    });

                    // CFArrayGetValueAtIndex returns a borrowed reference;
                    // CachedAxRef::from_borrowed retains it exactly once.
                    let Some(ax) = CachedAxRef::from_borrowed(window_ref) else {
                        continue;
                    };

                    let bounds = Bounds::new(x, y, width, height);
                    let capabilities = collect_ax_capabilities(window_ref);
                    let visibility =
                        super::observation::classify_visibility(role.as_deref(), &title, bounds);

                    // AXParent identity key for native-tab grouping proof.
                    let parent_key = get_ax_attribute(window_ref, "AXParent")
                        .ok()
                        .filter(|parent| !parent.is_null())
                        .map(|parent| {
                            let key = cf_hash(parent);
                            cf_release(parent);
                            key
                        });

                    ax_facts.push(AxCorrelationRow {
                        pid,
                        title: title.clone(),
                        bounds,
                        role: role.clone(),
                        focused: is_focused,
                        main: is_main,
                        parent_key,
                        declared_tab_group: None,
                    });

                    windows.push(StagedWindow {
                        app: app_name_str.clone(),
                        title,
                        bounds,
                        pid,
                        bundle_id: bundle_id.clone(),
                        app_path: app_path.clone(),
                        app_order: i,
                        window_index: j as usize,
                        is_frontmost_app,
                        is_focused,
                        is_main,
                        is_minimized,
                        is_on_current_space: true,
                        role,
                        subrole,
                        native_window_id: None,
                        native_id_confidence: NativeIdConfidence::Unavailable,
                        base_legacy_id,
                        ax: Some(ax),
                        capabilities,
                        search_visibility: visibility,
                        provider_match_key: None,
                    });
                }

                // Release windows_value - AXUIElementCopyAttributeValue returns an owned
                // CF object that we must release (the "Copy" in the name means we own it)
                cf_release(windows_value);
            }

            if let Some(focused_window) = focused_window {
                cf_release(focused_window);
            }
            if let Some(main_window) = main_window {
                cf_release(main_window);
            }

            // Release ax_app - AXUIElementCreateApplication returns an owned CF object
            cf_release(ax_app);
        }
    }

    correlate_and_append_cg(&mut windows, &ax_facts, &running_apps_by_pid);

    info!(window_count = windows.len(), "Listed windows");
    Ok(windows)
}

/// Correlate AX rows with CG rows (native ids + tab groups), then append the
/// unmatched CG rows as CG-only observations.
fn correlate_and_append_cg(
    windows: &mut Vec<StagedWindow>,
    ax_facts: &[AxCorrelationRow],
    running_apps_by_pid: &HashMap<i32, RunningAppMetadata>,
) {
    let Ok(cg_windows) = list_core_graphics_windows_all_spaces() else {
        return;
    };

    let cg_facts: Vec<CgCorrelationRow> = cg_windows
        .iter()
        .map(|cg| CgCorrelationRow {
            native_window_id: cg.native_window_id,
            pid: cg.pid,
            title: cg.title.clone(),
            bounds: cg.bounds,
        })
        .collect();
    let correlation = correlate(ax_facts, &cg_facts);

    // Apply verdicts to the staged AX rows (index-aligned prefix of `windows`).
    for (index, verdict) in correlation.verdicts.iter().enumerate() {
        let Some(row) = windows.get_mut(index) else {
            break;
        };
        row.native_window_id = verdict.native_window_id;
        row.native_id_confidence = verdict.confidence;
        if verdict.internal_tab_member {
            // Non-primary members of a proven native-tab group stay
            // observable but leave the ordinary listing.
            row.search_visibility = SearchVisibility::InternalOnly;
        }
    }

    let consumed: std::collections::HashSet<usize> =
        correlation.consumed_cg_indices.into_iter().collect();

    for (index, cg_window) in cg_windows.into_iter().enumerate() {
        if consumed.contains(&index) {
            continue;
        }
        let Some(app) = running_apps_by_pid.get(&cg_window.pid) else {
            continue;
        };
        if window_matches_existing_ax_row(windows, &cg_window) {
            continue;
        }

        let base_legacy_id = ((cg_window.pid as u32) << 16) | (cg_window.native_window_id & 0xffff);
        windows.push(StagedWindow {
            app: app.app.clone(),
            title: cg_window.title,
            bounds: cg_window.bounds,
            pid: cg_window.pid,
            bundle_id: app.bundle_id.clone(),
            app_path: app.app_path.clone(),
            app_order: app.app_order,
            window_index: index,
            is_frontmost_app: app.is_frontmost_app,
            is_focused: false,
            is_main: false,
            is_minimized: false,
            is_on_current_space: cg_window.is_on_current_space,
            role: None,
            subrole: None,
            native_window_id: Some(cg_window.native_window_id),
            native_id_confidence: NativeIdConfidence::UniquePublicCorrelation,
            base_legacy_id,
            ax: None,
            capabilities: WindowCapabilities::non_actionable("core_graphics_only"),
            search_visibility: SearchVisibility::Ordinary,
            provider_match_key: None,
        });
    }
}

fn window_matches_existing_ax_row(
    windows: &[StagedWindow],
    cg_window: &CoreGraphicsWindowInfo,
) -> bool {
    windows.iter().any(|window| {
        window.pid == cg_window.pid
            && window.title == cg_window.title
            && bounds_are_close(window.bounds, cg_window.bounds)
    })
}

fn bounds_are_close(left: Bounds, right: Bounds) -> bool {
    const TOLERANCE: i32 = 12;
    (left.x - right.x).abs() <= TOLERANCE
        && (left.y - right.y).abs() <= TOLERANCE
        && (left.width as i32 - right.width as i32).abs() <= TOLERANCE
        && (left.height as i32 - right.height as i32).abs() <= TOLERANCE
}

fn list_core_graphics_windows_all_spaces() -> Result<Vec<CoreGraphicsWindowInfo>> {
    const K_CG_NULL_WINDOW_ID: u32 = 0;
    const K_CG_WINDOW_LIST_OPTION_ALL: u32 = 0;
    const K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGWindowListCopyWindowInfo(
            option: u32,
            relative_to_window: u32,
        ) -> core_foundation::array::CFArrayRef;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFDictionaryGetValueIfPresent(
            the_dict: CFDictionaryRef,
            key: *const c_void,
            value: *mut *const c_void,
        ) -> u8;
    }

    let window_info_list = unsafe {
        CGWindowListCopyWindowInfo(
            K_CG_WINDOW_LIST_OPTION_ALL | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS,
            K_CG_NULL_WINDOW_ID,
        )
    };
    if window_info_list.is_null() {
        return Ok(Vec::new());
    }

    let info_array: CFArray = unsafe { CFArray::wrap_under_create_rule(window_info_list) };
    let k_owner_pid = CFString::new("kCGWindowOwnerPID");
    let k_window_number = CFString::new("kCGWindowNumber");
    let k_window_name = CFString::new("kCGWindowName");
    let k_window_bounds = CFString::new("kCGWindowBounds");
    let k_window_is_on_screen = CFString::new("kCGWindowIsOnscreen");
    let k_window_layer = CFString::new("kCGWindowLayer");
    let k_window_alpha = CFString::new("kCGWindowAlpha");

    let mut windows = Vec::new();
    for index in 0..info_array.len() {
        let Some(item_ref) = info_array.get(index) else {
            continue;
        };
        let dict_ref = *item_ref as CFDictionaryRef;
        if dict_ref.is_null() {
            continue;
        }
        if cf_number_i64(dict_ref, &k_window_layer) != Some(0) {
            continue;
        }
        if cf_number_f64(dict_ref, &k_window_alpha).is_some_and(|alpha| alpha <= 0.0) {
            continue;
        }
        let Some(pid) = cf_number_i64(dict_ref, &k_owner_pid).map(|value| value as i32) else {
            continue;
        };
        let Some(native_window_id) = cf_number_i64(dict_ref, &k_window_number)
            .filter(|value| *value >= 0)
            .map(|value| value as u32)
        else {
            continue;
        };
        let Some(bounds) = cf_bounds(dict_ref, &k_window_bounds) else {
            continue;
        };
        if bounds.width < 50 || bounds.height < 50 {
            continue;
        }
        let Some(title) = cf_string(dict_ref, &k_window_name).filter(|value| !value.is_empty())
        else {
            continue;
        };

        windows.push(CoreGraphicsWindowInfo {
            native_window_id,
            pid,
            title,
            bounds,
            is_on_current_space: cf_bool(dict_ref, &k_window_is_on_screen).unwrap_or(false),
        });

        if index > 5000 {
            break;
        }
    }

    unsafe fn dictionary_value(
        dict: CFDictionaryRef,
        key: &CFString,
        get_value: unsafe extern "C" fn(CFDictionaryRef, *const c_void, *mut *const c_void) -> u8,
    ) -> Option<CoreFoundationTypeRef> {
        let mut value: *const c_void = std::ptr::null();
        if get_value(dict, key.as_concrete_TypeRef() as *const c_void, &mut value) == 0
            || value.is_null()
        {
            None
        } else {
            Some(value as CoreFoundationTypeRef)
        }
    }

    fn value_for(dict: CFDictionaryRef, key: &CFString) -> Option<CoreFoundationTypeRef> {
        unsafe { dictionary_value(dict, key, CFDictionaryGetValueIfPresent) }
    }

    fn cf_number_i64(dict: CFDictionaryRef, key: &CFString) -> Option<i64> {
        use core_foundation::number::CFNumber;
        let value = value_for(dict, key)?;
        let number = unsafe { CFNumber::wrap_under_get_rule(value as _) };
        number.to_i64()
    }

    fn cf_number_f64(dict: CFDictionaryRef, key: &CFString) -> Option<f64> {
        use core_foundation::number::CFNumber;
        let value = value_for(dict, key)?;
        let number = unsafe { CFNumber::wrap_under_get_rule(value as _) };
        number.to_f64()
    }

    fn cf_bool(dict: CFDictionaryRef, key: &CFString) -> Option<bool> {
        use core_foundation::boolean::CFBoolean;
        let value = value_for(dict, key)?;
        Some(unsafe { CFBoolean::wrap_under_get_rule(value as _) }.into())
    }

    fn cf_string(dict: CFDictionaryRef, key: &CFString) -> Option<String> {
        let value = value_for(dict, key)?;
        let string = unsafe { CFString::wrap_under_get_rule(value as _) };
        Some(string.to_string())
    }

    fn cf_bounds(dict: CFDictionaryRef, key: &CFString) -> Option<Bounds> {
        use core_foundation::dictionary::CFDictionary;
        let value = value_for(dict, key)?;
        let bounds_dict = unsafe {
            CFDictionary::<CFString, core_foundation::base::CFType>::wrap_under_get_rule(value as _)
        };
        let x = bounds_dict
            .find(CFString::new("X"))
            .and_then(|value| unsafe {
                core_foundation::number::CFNumber::wrap_under_get_rule(value.as_CFTypeRef() as _)
                    .to_f64()
            })?;
        let y = bounds_dict
            .find(CFString::new("Y"))
            .and_then(|value| unsafe {
                core_foundation::number::CFNumber::wrap_under_get_rule(value.as_CFTypeRef() as _)
                    .to_f64()
            })?;
        let width = bounds_dict
            .find(CFString::new("Width"))
            .and_then(|value| unsafe {
                core_foundation::number::CFNumber::wrap_under_get_rule(value.as_CFTypeRef() as _)
                    .to_f64()
            })?;
        let height = bounds_dict
            .find(CFString::new("Height"))
            .and_then(|value| unsafe {
                core_foundation::number::CFNumber::wrap_under_get_rule(value.as_CFTypeRef() as _)
                    .to_f64()
            })?;

        Some(Bounds::new(
            x.round() as i32,
            y.round() as i32,
            width.round() as u32,
            height.round() as u32,
        ))
    }

    Ok(windows)
}

unsafe fn nsstring_to_string(value: *mut objc::runtime::Object) -> Option<String> {
    use objc::{msg_send, sel, sel_impl};
    if value.is_null() {
        return None;
    }
    let utf8: *const i8 = msg_send![value, UTF8String];
    if utf8.is_null() {
        return None;
    }
    std::ffi::CStr::from_ptr(utf8)
        .to_str()
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

unsafe fn nsurl_to_pathbuf(url: *mut objc::runtime::Object) -> Option<PathBuf> {
    use objc::{msg_send, sel, sel_impl};
    if url.is_null() {
        return None;
    }
    let path: *mut objc::runtime::Object = msg_send![url, path];
    nsstring_to_string(path).map(PathBuf::from)
}

/// Get the PID of the application that owns the menu bar.
///
/// When Script Kit (an accessory/LSUIElement app) is active, it does NOT take
/// menu bar ownership. The previously active "regular" app still owns the menu bar.
/// This is exactly what we need for window actions - we want to act on the
/// window that was focused before Script Kit was shown.
///
/// # Returns
/// The process identifier (PID) of the menu bar owning application.
///
/// # Errors
/// Returns error if no menu bar owner is found or if the PID is invalid.
#[instrument]
pub fn get_menu_bar_owner_pid() -> Result<i32> {
    // SAFETY: Standard objc messaging to NSWorkspace singleton. All returned
    // pointers are null-checked. CStr::from_ptr reads from a valid NSString UTF8String.
    unsafe {
        use objc::runtime::{Class, Object};
        use objc::{msg_send, sel, sel_impl};

        let workspace_class = Class::get("NSWorkspace").context("Failed to get NSWorkspace")?;
        let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
        let menu_owner: *mut Object = msg_send![workspace, menuBarOwningApplication];

        if menu_owner.is_null() {
            bail!("No menu bar owning application found");
        }

        let pid: i32 = msg_send![menu_owner, processIdentifier];

        if pid <= 0 {
            bail!("Invalid process identifier for menu bar owner");
        }

        // Log for debugging only: this can be polled at high frequency by the
        // snap monitor and should not appear in normal AI log mode.
        let name: *mut Object = msg_send![menu_owner, localizedName];
        let name_str = if !name.is_null() {
            let utf8: *const i8 = msg_send![name, UTF8String];
            if !utf8.is_null() {
                std::ffi::CStr::from_ptr(utf8).to_str().unwrap_or("unknown")
            } else {
                "unknown"
            }
        } else {
            "unknown"
        };

        debug!(pid, app_name = name_str, "Got menu bar owner");
        Ok(pid)
    }
}

/// Get the frontmost window of the menu bar owning application.
///
/// This is the key function for window actions from Script Kit. When the user
/// executes "Tile Window Left" etc., we want to act on the window they were
/// using before invoking Script Kit, not Script Kit's own window.
///
/// Since Script Kit is an LSUIElement (accessory app), it doesn't take menu bar
/// ownership. The menu bar owner is the previously active app.
///
/// # Window Selection Strategy
///
/// 1. First try AXFocusedWindow - the actual focused window of the app
/// 2. If that fails, try AXMainWindow - the app's designated "main" window
/// 3. Fall back to first window in AXWindows array if neither works
///
/// This is more accurate than just picking the first window with matching pid,
/// which can return the wrong window if the app has multiple windows open.
///
/// # Returns
/// The focused/main window of the menu bar owning application, or None if
/// no windows are found.
#[instrument]
pub fn get_frontmost_window_of_previous_app() -> Result<Option<WindowInfo>> {
    let target_pid = get_menu_bar_owner_pid()?;

    // SAFETY: AXUIElementCreateApplication is called with a valid PID. All AX attribute
    // queries use safe wrappers. CF objects are retained when borrowed from arrays
    // (CFArrayGetValueAtIndex) and released when no longer needed.
    unsafe {
        // Create AX element for the target application
        let ax_app = AXUIElementCreateApplication(target_pid);
        if ax_app.is_null() {
            warn!(target_pid, "Failed to create AXUIElement for app");
            return Ok(None);
        }

        // Strategy 1: Try to get the focused window (most accurate)
        let focused_window = get_ax_attribute(ax_app as AXUIElementRef, "AXFocusedWindow")
            .ok()
            .filter(|&w| !w.is_null());

        // Strategy 2: Fall back to main window
        let target_window = focused_window.or_else(|| {
            get_ax_attribute(ax_app as AXUIElementRef, "AXMainWindow")
                .ok()
                .filter(|&w| !w.is_null())
        });

        // Strategy 3: Fall back to first window in AXWindows list
        let target_window = target_window.or_else(|| {
            if let Ok(windows_value) = get_ax_attribute(ax_app as AXUIElementRef, "AXWindows") {
                let count = CFArrayGetCount(windows_value as CFArrayRef);
                if count > 0 {
                    let window = CFArrayGetValueAtIndex(windows_value as CFArrayRef, 0);
                    // Retain the window ref since CFArrayGetValueAtIndex returns borrowed
                    let retained = cf_retain(window);
                    cf_release(windows_value);
                    Some(retained)
                } else {
                    cf_release(windows_value);
                    None
                }
            } else {
                None
            }
        });

        // Release ax_app - we're done with it
        cf_release(ax_app);

        // If we found a target window, reconcile it with the registry so the
        // returned WindowInfo always carries a current-generation handle.
        if let Some(window_ref) = target_window {
            let ax_window = window_ref as AXUIElementRef;

            // Get window attributes
            let title = get_window_string_attribute(ax_window, "AXTitle").unwrap_or_default();
            let (x, y) = get_window_position(ax_window).unwrap_or((0, 0));
            let (width, height) = get_window_size(ax_window).unwrap_or((0, 0));

            // Get app name for logging
            let app_name = get_app_name_for_pid(target_pid);

            // The AX resolution above produced an OWNED reference (Copy API
            // or explicit retain); transfer ownership to the registry.
            let Some(ax) = CachedAxRef::from_owned(ax_window) else {
                warn!(target_pid, "Previous-app window ref was null");
                return Ok(None);
            };

            let window_info = super::registry::upsert_previous_app_window(
                target_pid,
                app_name.clone(),
                title.clone(),
                Bounds::new(x, y, width, height),
                ax,
            );

            debug!(
                window_id = window_info.id,
                app = %app_name,
                title = %title,
                "Found focused/main window of previous app via AX"
            );

            Ok(Some(window_info))
        } else {
            warn!(
                target_pid,
                "No focused or main window found for menu bar owner"
            );
            Ok(None)
        }
    }
}

/// Get the localized app name for a given PID.
fn get_app_name_for_pid(pid: i32) -> String {
    // SAFETY: Standard objc messaging to NSWorkspace/NSRunningApplication.
    // All pointers are null-checked before dereferencing.
    unsafe {
        use objc::runtime::{Class, Object};
        use objc::{msg_send, sel, sel_impl};

        if let Some(workspace_class) = Class::get("NSWorkspace") {
            let workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
            let running_apps: *mut Object = msg_send![workspace, runningApplications];
            let app_count: usize = msg_send![running_apps, count];

            for i in 0..app_count {
                let app: *mut Object = msg_send![running_apps, objectAtIndex: i];
                let app_pid: i32 = msg_send![app, processIdentifier];

                if app_pid == pid {
                    let app_name: *mut Object = msg_send![app, localizedName];
                    if !app_name.is_null() {
                        let utf8: *const i8 = msg_send![app_name, UTF8String];
                        if !utf8.is_null() {
                            return std::ffi::CStr::from_ptr(utf8)
                                .to_str()
                                .unwrap_or("Unknown")
                                .to_string();
                        }
                    }
                    break;
                }
            }
        }

        "Unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_check_does_not_panic() {
        // This test verifies the permission check doesn't panic
        let _has_permission = has_accessibility_permission();
    }

    #[test]
    #[ignore] // Requires accessibility permission
    fn test_list_windows() {
        let windows = list_windows().expect("Should list windows");
        println!("Found {} windows:", windows.len());
        for window in &windows {
            println!(
                "  [{:08x}] {}: {} ({:?})",
                window.id, window.app, window.title, window.bounds
            );
        }
    }
}

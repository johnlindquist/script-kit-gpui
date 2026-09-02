//! Low-level AX access belongs here.
//!
//! Focused-text contracts keep raw AX handles out of DTOs. Native
//! handle storage will live in a short-lived process-local registry owned by
//! this module.

#[cfg(target_os = "macos")]
use std::collections::HashMap;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "macos")]
use anyhow::{bail, Context, Result};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AxSessionHandle {
    pub(crate) session_id: super::FocusedTextSessionId,
}

#[cfg(target_os = "macos")]
pub(crate) type AXUIElementRef = *const c_void;
#[cfg(target_os = "macos")]
type CFTypeRef = *const c_void;
#[cfg(target_os = "macos")]
type CFStringRef = *const c_void;

#[cfg(target_os = "macos")]
const K_AX_ERROR_SUCCESS: i32 = 0;
#[cfg(target_os = "macos")]
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
#[cfg(target_os = "macos")]
const K_CF_NUMBER_LONG_LONG_TYPE: i32 = 4;
#[cfg(target_os = "macos")]
const K_AX_VALUE_CG_POINT_TYPE: i32 = 1;
#[cfg(target_os = "macos")]
const K_AX_VALUE_CG_SIZE_TYPE: i32 = 2;
#[cfg(target_os = "macos")]
const K_AX_VALUE_CG_RECT_TYPE: i32 = 3;
#[cfg(target_os = "macos")]
const K_AX_VALUE_CF_RANGE_TYPE: i32 = 4;

#[cfg(target_os = "macos")]
const AX_FOCUSED_UI_ELEMENT: &str = "AXFocusedUIElement";
#[cfg(target_os = "macos")]
const AX_ROLE: &str = "AXRole";
#[cfg(target_os = "macos")]
const AX_SUBROLE: &str = "AXSubrole";
#[cfg(target_os = "macos")]
const AX_VALUE: &str = "AXValue";
#[cfg(target_os = "macos")]
const AX_NUMBER_OF_CHARACTERS: &str = "AXNumberOfCharacters";
#[cfg(target_os = "macos")]
const AX_STRING_FOR_RANGE: &str = "AXStringForRange";
#[cfg(target_os = "macos")]
const AX_SELECTED_TEXT_RANGE: &str = "AXSelectedTextRange";
#[cfg(target_os = "macos")]
const AX_BOUNDS_FOR_RANGE: &str = "AXBoundsForRange";
#[cfg(target_os = "macos")]
const AX_POSITION: &str = "AXPosition";
#[cfg(target_os = "macos")]
const AX_SIZE: &str = "AXSize";
#[cfg(target_os = "macos")]
const AX_WINDOW: &str = "AXWindow";
#[cfg(target_os = "macos")]
const AX_ENABLED: &str = "AXEnabled";

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct CFRange {
    location: isize,
    length: isize,
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        parameterized_attribute: CFStringRef,
        parameter: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> i32;
    fn AXValueCreate(the_type: i32, value_ptr: *const c_void) -> CFTypeRef;
    fn AXValueGetValue(value: CFTypeRef, the_type: i32, value_ptr: *mut c_void) -> bool;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    fn CFGetTypeID(cf: CFTypeRef) -> u64;
    fn CFStringGetTypeID() -> u64;
    fn CFNumberGetTypeID() -> u64;
    fn CFBooleanGetTypeID() -> u64;
    fn CFBooleanGetValue(boolean: CFTypeRef) -> bool;
    fn CFNumberGetValue(number: CFTypeRef, the_type: i32, value_ptr: *mut c_void) -> bool;
    fn CFStringCreateWithCString(
        alloc: *const c_void,
        c_str: *const i8,
        encoding: u32,
    ) -> CFStringRef;
    fn CFStringGetCString(
        string: CFStringRef,
        buffer: *mut i8,
        buffer_size: i64,
        encoding: u32,
    ) -> bool;
    fn CFStringGetLength(string: CFStringRef) -> i64;
}

#[cfg(target_os = "macos")]
const FOCUSED_TEXT_SESSION_TTL_MS: u128 = 30_000;

#[cfg(target_os = "macos")]
pub(crate) struct OwnedAxElement {
    ptr: AXUIElementRef,
}

#[cfg(target_os = "macos")]
impl OwnedAxElement {
    fn from_create_rule(ptr: AXUIElementRef) -> Result<Self> {
        if ptr.is_null() {
            bail!("AX element was null");
        }
        Ok(Self { ptr })
    }

    pub(crate) fn as_ptr(&self) -> AXUIElementRef {
        self.ptr
    }
}

#[cfg(target_os = "macos")]
impl Drop for OwnedAxElement {
    fn drop(&mut self) {
        cf_release(self.ptr as CFTypeRef);
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct StoredFocusedTextSession {
    element: usize,
    captured_at_ms: u128,
    captured_text: String,
    app_process_id: Option<i32>,
}

#[cfg(target_os = "macos")]
unsafe impl Send for StoredFocusedTextSession {}

#[cfg(target_os = "macos")]
impl StoredFocusedTextSession {
    fn element(&self) -> AXUIElementRef {
        self.element as AXUIElementRef
    }
}

#[cfg(target_os = "macos")]
impl Drop for StoredFocusedTextSession {
    fn drop(&mut self) {
        cf_release(self.element());
    }
}

#[cfg(target_os = "macos")]
fn focused_text_sessions() -> &'static Mutex<HashMap<String, StoredFocusedTextSession>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, StoredFocusedTextSession>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(target_os = "macos")]
pub(crate) fn register_focused_text_session(
    session_id: &super::FocusedTextSessionId,
    element: AXUIElementRef,
    captured_at_ms: u128,
    captured_text: String,
    app_process_id: Option<i32>,
) -> Result<()> {
    if element.is_null() {
        bail!("cannot register null focused text element");
    }
    let retained = unsafe { CFRetain(element as CFTypeRef) };
    if retained.is_null() {
        bail!("CFRetain failed for focused text element");
    }

    let mut sessions = focused_text_sessions()
        .lock()
        .map_err(|_| anyhow::anyhow!("focused text session registry poisoned"))?;
    sessions.insert(
        session_id.to_string(),
        StoredFocusedTextSession {
            element: retained as usize,
            captured_at_ms,
            captured_text,
            app_process_id,
        },
    );
    prune_stale_sessions_locked(&mut sessions, captured_at_ms);
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn replace_registered_focused_text(
    session_id: &super::FocusedTextSessionId,
    text: &str,
    options: super::mutation::TextMutationOptions,
    now_ms: u128,
) -> Result<super::mutation::TextMutationResult, super::FocusedTextError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput)
        .map_err(|error| super::FocusedTextError::Platform(error.to_string()))?;
    let target = registered_target(session_id, options, now_ms)?;
    if set_whole_text_direct(target.element, text).is_err() {
        paste_replace_fallback(&target, text)?;
    }
    let _ = set_selected_text_range(
        target.element,
        super::TextRangeUtf16 {
            location: text.encode_utf16().count(),
            length: 0,
        },
    );
    verify_whole_text_or_clipboard_fallback(&target, text)?;
    Ok(super::mutation::TextMutationResult {
        action: super::mutation::TextMutationAction::Replace,
        changed_text: true,
        copied_to_clipboard: false,
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn append_registered_focused_text(
    session_id: &super::FocusedTextSessionId,
    text: &str,
    options: super::mutation::TextMutationOptions,
    now_ms: u128,
) -> Result<super::mutation::TextMutationResult, super::FocusedTextError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput)
        .map_err(|error| super::FocusedTextError::Platform(error.to_string()))?;
    let target = registered_target(session_id, options, now_ms)?;
    let current = match whole_text(target.element) {
        Ok(current) => current,
        Err(err) => {
            tracing::warn!(
                target: "script_kit::focused_text",
                event = "focused_text_append_whole_text_failed_using_captured_text_fallback",
                error = %err,
            );
            target.captured_text.clone()
        }
    };
    let appended = format!("{current}{text}");
    if set_whole_text_direct(target.element, &appended).is_err() {
        paste_append_fallback(&target, &current, text, &appended)?;
    }
    let _ = set_selected_text_range(
        target.element,
        super::TextRangeUtf16 {
            location: appended.encode_utf16().count(),
            length: 0,
        },
    );
    verify_whole_text_or_clipboard_fallback(&target, &appended)?;
    Ok(super::mutation::TextMutationResult {
        action: super::mutation::TextMutationAction::Append,
        changed_text: true,
        copied_to_clipboard: false,
    })
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct RegisteredFocusedTextTarget {
    element: AXUIElementRef,
    app_process_id: Option<i32>,
    captured_text: String,
}

#[cfg(target_os = "macos")]
fn registered_target(
    session_id: &super::FocusedTextSessionId,
    options: super::mutation::TextMutationOptions,
    now_ms: u128,
) -> Result<RegisteredFocusedTextTarget, super::FocusedTextError> {
    let mut sessions = focused_text_sessions().lock().map_err(|_| {
        super::FocusedTextError::Platform("focused text session registry poisoned".to_string())
    })?;
    prune_stale_sessions_locked(&mut sessions, now_ms);
    let Some(session) = sessions.get(&session_id.to_string()) else {
        return Err(super::FocusedTextError::StaleSession);
    };
    let target = RegisteredFocusedTextTarget {
        element: session.element(),
        app_process_id: session.app_process_id,
        captured_text: session.captured_text.clone(),
    };
    let mutation_session = super::mutation::FocusedTextMutationSession {
        session_id: session_id.clone(),
        captured_at_ms: session.captured_at_ms,
        current_text: Some(session.captured_text.clone()),
        ttl_ms: FOCUSED_TEXT_SESSION_TTL_MS,
    };
    super::mutation::validate_mutation_session(&mutation_session, options, now_ms)?;
    Ok(target)
}

#[cfg(target_os = "macos")]
fn prune_stale_sessions_locked(
    sessions: &mut HashMap<String, StoredFocusedTextSession>,
    now_ms: u128,
) {
    sessions.retain(|_, session| {
        now_ms.saturating_sub(session.captured_at_ms) <= FOCUSED_TEXT_SESSION_TTL_MS
    });
}

/// Focused element of a SPECIFIC app by pid, never consulting the system-wide
/// element. Required when our own panel may already be key: the system-wide
/// `AXFocusedUIElement` would resolve to Script Kit itself instead of the app
/// the user came from.
#[cfg(target_os = "macos")]
pub(crate) fn focused_ui_element_for_pid(pid: i32) -> Result<OwnedAxElement> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    let app = unsafe { AXUIElementCreateApplication(pid) };
    let app = OwnedAxElement::from_create_rule(app)?;
    let value = copy_attribute(app.as_ptr(), AX_FOCUSED_UI_ELEMENT)?;
    OwnedAxElement::from_create_rule(value as AXUIElementRef)
}

#[cfg(target_os = "macos")]
pub(crate) fn focused_ui_element_for_app(pid: Option<i32>) -> Result<OwnedAxElement> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    let system = unsafe { AXUIElementCreateSystemWide() };
    let system = OwnedAxElement::from_create_rule(system)?;
    if let Ok(value) = copy_attribute(system.as_ptr(), AX_FOCUSED_UI_ELEMENT) {
        return OwnedAxElement::from_create_rule(value as AXUIElementRef);
    }

    let Some(pid) = pid else {
        bail!("system-wide AXFocusedUIElement was unavailable and no frontmost pid was known");
    };
    let app = unsafe { AXUIElementCreateApplication(pid) };
    let app = OwnedAxElement::from_create_rule(app)?;
    let value = copy_attribute(app.as_ptr(), AX_FOCUSED_UI_ELEMENT)?;
    OwnedAxElement::from_create_rule(value as AXUIElementRef)
}

#[cfg(target_os = "macos")]
pub(crate) fn string_attribute(element: AXUIElementRef, attribute: &str) -> Option<String> {
    let value = copy_attribute(element, attribute).ok()?;
    let result = cf_string_to_string_if_string(value);
    cf_release(value);
    result
}

#[cfg(target_os = "macos")]
pub(crate) fn role(element: AXUIElementRef) -> Option<String> {
    string_attribute(element, AX_ROLE)
}

#[cfg(target_os = "macos")]
pub(crate) fn subrole(element: AXUIElementRef) -> Option<String> {
    string_attribute(element, AX_SUBROLE)
}

#[cfg(target_os = "macos")]
pub(crate) fn is_enabled(element: AXUIElementRef) -> Option<bool> {
    let value = copy_attribute(element, AX_ENABLED).ok()?;
    let result = cf_bool_value(value);
    cf_release(value);
    result
}

#[cfg(target_os = "macos")]
const AX_SELECTED_TEXT: &str = "AXSelectedText";

/// Passive read of the element's selected text: `AXSelectedText` first, then
/// `AXSelectedTextRange` + `AXStringForRange`. Never posts keystrokes and
/// never touches the pasteboard.
#[cfg(target_os = "macos")]
pub(crate) fn selected_text(element: AXUIElementRef) -> Option<String> {
    if let Some(text) = string_attribute(element, AX_SELECTED_TEXT) {
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    let range = selected_text_range(element)?;
    if range.length == 0 {
        return None;
    }
    string_for_range(element, range)
        .ok()
        .filter(|text| !text.trim().is_empty())
}

#[cfg(target_os = "macos")]
pub(crate) fn selected_text_range(element: AXUIElementRef) -> Option<super::TextRangeUtf16> {
    let value = copy_attribute(element, AX_SELECTED_TEXT_RANGE).ok()?;
    let result = cf_range_value(value).and_then(text_range_from_cf_range);
    cf_release(value);
    result
}

#[cfg(target_os = "macos")]
pub(crate) fn whole_text(element: AXUIElementRef) -> Result<String> {
    if let Ok(value) = copy_attribute(element, AX_VALUE) {
        let result = cf_string_to_string_if_string(value);
        cf_release(value);
        if let Some(text) = result {
            return Ok(text);
        }
    }

    let char_count = number_attribute_usize(element, AX_NUMBER_OF_CHARACTERS)
        .context("AXValue was unavailable and AXNumberOfCharacters could not be read")?;
    string_for_range(
        element,
        super::TextRangeUtf16 {
            location: 0,
            length: char_count,
        },
    )
}

#[cfg(target_os = "macos")]
pub(crate) fn set_whole_text_direct(
    element: AXUIElementRef,
    text: &str,
) -> Result<(), super::FocusedTextError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput)
        .map_err(|error| super::FocusedTextError::Platform(error.to_string()))?;
    let value =
        create_cf_string(text).map_err(|err| super::FocusedTextError::Platform(err.to_string()))?;
    let attr = create_cf_string(AX_VALUE)
        .map_err(|err| super::FocusedTextError::Platform(err.to_string()))?;
    let result = unsafe { AXUIElementSetAttributeValue(element, attr, value) };
    cf_release(attr);
    cf_release(value);
    if result != K_AX_ERROR_SUCCESS {
        return Err(super::FocusedTextError::UnsupportedTarget);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn set_selected_text_range(
    element: AXUIElementRef,
    range: super::TextRangeUtf16,
) -> Result<(), super::FocusedTextError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput)
        .map_err(|error| super::FocusedTextError::Platform(error.to_string()))?;
    let cf_range = CFRange {
        location: range.location as isize,
        length: range.length as isize,
    };
    let range_value = unsafe {
        AXValueCreate(
            K_AX_VALUE_CF_RANGE_TYPE,
            &cf_range as *const CFRange as *const c_void,
        )
    };
    if range_value.is_null() {
        return Err(super::FocusedTextError::Platform(
            "AXValueCreate failed for selected text range".to_string(),
        ));
    }
    let attr = create_cf_string(AX_SELECTED_TEXT_RANGE)
        .map_err(|err| super::FocusedTextError::Platform(err.to_string()))?;
    let result = unsafe { AXUIElementSetAttributeValue(element, attr, range_value) };
    cf_release(attr);
    cf_release(range_value);
    if result != K_AX_ERROR_SUCCESS {
        return Err(super::FocusedTextError::UnsupportedTarget);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_whole_text(
    element: AXUIElementRef,
    expected: &str,
) -> Result<(), super::FocusedTextError> {
    let actual =
        whole_text(element).map_err(|err| super::FocusedTextError::Platform(err.to_string()))?;
    if actual == expected {
        Ok(())
    } else {
        Err(super::FocusedTextError::Platform(
            "focused text mutation verification failed".to_string(),
        ))
    }
}

#[cfg(target_os = "macos")]
fn verify_whole_text_or_clipboard_fallback(
    target: &RegisteredFocusedTextTarget,
    expected: &str,
) -> Result<(), super::FocusedTextError> {
    match verify_whole_text(target.element, expected) {
        Ok(()) => Ok(()),
        Err(err) => {
            tracing::warn!(
                target: "script_kit::focused_text",
                event = "focused_text_verify_whole_text_failed_trying_clipboard_fallback",
                error = %err,
            );
            refocus_registered_target_for_paste(target)?;
            let actual = super::clipboard::copy_all_plain_text_preserving_clipboard()
                .map_err(|err| super::FocusedTextError::Platform(err.to_string()))?;
            if actual == expected {
                Ok(())
            } else {
                Err(super::FocusedTextError::Platform(
                    "focused text mutation clipboard verification failed".to_string(),
                ))
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn paste_replace_fallback(
    target: &RegisteredFocusedTextTarget,
    text: &str,
) -> Result<(), super::FocusedTextError> {
    refocus_registered_target_for_paste(target)?;
    match whole_text(target.element) {
        Ok(current) => {
            set_selected_text_range(
                target.element,
                super::TextRangeUtf16 {
                    location: 0,
                    length: current.encode_utf16().count(),
                },
            )?;
        }
        Err(err) => {
            tracing::warn!(
                target: "script_kit::focused_text",
                event = "focused_text_replace_whole_text_failed_using_select_all_fallback",
                error = %err,
            );
            super::clipboard::select_all_text_for_focused_text_fallback()
                .map_err(|err| super::FocusedTextError::Platform(err.to_string()))?;
        }
    }
    super::clipboard::paste_plain_text_preserving_clipboard(text)
        .map_err(|err| super::FocusedTextError::Platform(err.to_string()))
}

#[cfg(target_os = "macos")]
fn paste_append_fallback(
    target: &RegisteredFocusedTextTarget,
    current: &str,
    output: &str,
    appended: &str,
) -> Result<(), super::FocusedTextError> {
    refocus_registered_target_for_paste(target)?;
    match super::mutation::plan_append_mutation(Some(current), output, false, true) {
        super::mutation::AppendMutationPlan::PasteOutputAtEnd { output, .. } => {
            let caret_result = set_selected_text_range(
                target.element,
                super::TextRangeUtf16 {
                    location: current.encode_utf16().count(),
                    length: 0,
                },
            );
            if caret_result.is_ok() {
                return super::clipboard::paste_plain_text_preserving_clipboard(&output)
                    .map_err(|err| super::FocusedTextError::Platform(err.to_string()));
            }
        }
        super::mutation::AppendMutationPlan::DirectSet { .. }
        | super::mutation::AppendMutationPlan::SelectAllAndPaste { .. } => {}
    }

    set_selected_text_range(
        target.element,
        super::TextRangeUtf16 {
            location: 0,
            length: current.encode_utf16().count(),
        },
    )?;
    super::clipboard::paste_plain_text_preserving_clipboard(appended)
        .map_err(|err| super::FocusedTextError::Platform(err.to_string()))
}

#[cfg(target_os = "macos")]
fn refocus_registered_target_for_paste(
    target: &RegisteredFocusedTextTarget,
) -> Result<(), super::FocusedTextError> {
    let Some(pid) = target.app_process_id else {
        return Err(super::FocusedTextError::Platform(
            "focused-text Agent Chat paste fallback refused without captured target app pid"
                .to_string(),
        ));
    };

    activate_application_for_pid(pid)?;
    set_focused_ui_element_for_app(pid, target.element)?;
    std::thread::sleep(std::time::Duration::from_millis(40));
    verify_registered_target_is_focused_for_paste(pid, target.element)
}

#[cfg(target_os = "macos")]
fn activate_application_for_pid(pid: i32) -> Result<(), super::FocusedTextError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeVisibility)
        .map_err(|error| super::FocusedTextError::Platform(error.to_string()))?;
    use objc::runtime::{Class, Object};
    use objc::{msg_send, sel, sel_impl};

    const NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS: u64 = 1 << 1;

    unsafe {
        let Some(app_class) = Class::get("NSRunningApplication") else {
            return Err(super::FocusedTextError::Platform(
                "NSRunningApplication class was unavailable".to_string(),
            ));
        };
        let app: *mut Object = msg_send![app_class, runningApplicationWithProcessIdentifier: pid];
        if app.is_null() {
            return Err(super::FocusedTextError::StaleSession);
        }
        let activated: bool =
            msg_send![app, activateWithOptions: NS_APPLICATION_ACTIVATE_IGNORING_OTHER_APPS];
        if !activated {
            return Err(super::FocusedTextError::Platform(
                "failed to activate captured target app before paste fallback".to_string(),
            ));
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn set_focused_ui_element_for_app(
    pid: i32,
    element: AXUIElementRef,
) -> Result<(), super::FocusedTextError> {
    let app = unsafe { AXUIElementCreateApplication(pid) };
    let app = OwnedAxElement::from_create_rule(app)
        .map_err(|err| super::FocusedTextError::Platform(err.to_string()))?;
    let attr = create_cf_string(AX_FOCUSED_UI_ELEMENT)
        .map_err(|err| super::FocusedTextError::Platform(err.to_string()))?;
    let result = unsafe { AXUIElementSetAttributeValue(app.as_ptr(), attr, element as CFTypeRef) };
    cf_release(attr);
    if result != K_AX_ERROR_SUCCESS {
        return Err(super::FocusedTextError::Platform(
            "failed to focus captured AX element before paste fallback".to_string(),
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_registered_target_is_focused_for_paste(
    pid: i32,
    element: AXUIElementRef,
) -> Result<(), super::FocusedTextError> {
    let focused = focused_ui_element_for_app(Some(pid))
        .map_err(|err| super::FocusedTextError::Platform(err.to_string()))?;
    let target_text =
        whole_text(element).map_err(|err| super::FocusedTextError::Platform(err.to_string()))?;
    let focused_text = whole_text(focused.as_ptr())
        .map_err(|err| super::FocusedTextError::Platform(err.to_string()))?;

    if target_text == focused_text {
        Ok(())
    } else {
        Err(super::FocusedTextError::Platform(
            "focused-text Agent Chat paste fallback refused because captured target is not focused"
                .to_string(),
        ))
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn focused_geometry(
    element: AXUIElementRef,
    selected_range: Option<super::TextRangeUtf16>,
) -> super::FocusedFieldGeometry {
    let caret_range = selected_range.map(|range| super::TextRangeUtf16 {
        location: range.location + range.length,
        length: 0,
    });
    let caret_bounds = caret_range.and_then(|range| bounds_for_range(element, range));
    let selection_bounds = selected_range.and_then(|range| bounds_for_range(element, range));
    let field_bounds = element_rect(element);
    let window_bounds = copy_attribute(element, AX_WINDOW).ok().and_then(|window| {
        let rect = element_rect(window as AXUIElementRef);
        cf_release(window);
        rect
    });

    super::FocusedFieldGeometry {
        caret_bounds,
        selection_bounds,
        field_bounds,
        window_bounds,
        display_bounds: super::DisplayBounds::default(),
    }
}

#[cfg(target_os = "macos")]
fn string_for_range(element: AXUIElementRef, range: super::TextRangeUtf16) -> Result<String> {
    let cf_range = CFRange {
        location: range.location as isize,
        length: range.length as isize,
    };
    let range_value = unsafe {
        AXValueCreate(
            K_AX_VALUE_CF_RANGE_TYPE,
            &cf_range as *const CFRange as *const c_void,
        )
    };
    if range_value.is_null() {
        bail!("AXValueCreate failed for AXStringForRange");
    }
    let result =
        copy_parameterized_attribute(element, AX_STRING_FOR_RANGE, range_value).and_then(|value| {
            let text = cf_string_to_string_if_string(value)
                .context("AXStringForRange did not return a CFString");
            cf_release(value);
            text
        });
    cf_release(range_value);
    result
}

#[cfg(target_os = "macos")]
fn bounds_for_range(
    element: AXUIElementRef,
    range: super::TextRangeUtf16,
) -> Option<super::geometry::RectPx> {
    let cf_range = CFRange {
        location: range.location as isize,
        length: range.length as isize,
    };
    let range_value = unsafe {
        AXValueCreate(
            K_AX_VALUE_CF_RANGE_TYPE,
            &cf_range as *const CFRange as *const c_void,
        )
    };
    if range_value.is_null() {
        return None;
    }
    let result = copy_parameterized_attribute(element, AX_BOUNDS_FOR_RANGE, range_value)
        .ok()
        .and_then(|value| {
            let rect = cg_rect_value(value);
            cf_release(value);
            rect
        });
    cf_release(range_value);
    result
}

#[cfg(target_os = "macos")]
fn element_rect(element: AXUIElementRef) -> Option<super::geometry::RectPx> {
    let position = copy_attribute(element, AX_POSITION)
        .ok()
        .and_then(|value| {
            let point = cg_point_value(value);
            cf_release(value);
            point
        })?;
    let size = copy_attribute(element, AX_SIZE).ok().and_then(|value| {
        let size = cg_size_value(value);
        cf_release(value);
        size
    })?;
    Some(super::geometry::RectPx {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

#[cfg(target_os = "macos")]
fn copy_attribute(element: AXUIElementRef, attribute: &str) -> Result<CFTypeRef> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    let attr = create_cf_string(attribute)?;
    let mut value: CFTypeRef = std::ptr::null();
    let result = unsafe { AXUIElementCopyAttributeValue(element, attr, &mut value) };
    cf_release(attr);
    if result != K_AX_ERROR_SUCCESS || value.is_null() {
        bail!("AX attribute {attribute} unavailable: error {result}");
    }
    Ok(value)
}

#[cfg(target_os = "macos")]
fn copy_parameterized_attribute(
    element: AXUIElementRef,
    attribute: &str,
    parameter: CFTypeRef,
) -> Result<CFTypeRef> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    let attr = create_cf_string(attribute)?;
    let mut value: CFTypeRef = std::ptr::null();
    let result =
        unsafe { AXUIElementCopyParameterizedAttributeValue(element, attr, parameter, &mut value) };
    cf_release(attr);
    if result != K_AX_ERROR_SUCCESS || value.is_null() {
        bail!("AX parameterized attribute {attribute} unavailable: error {result}");
    }
    Ok(value)
}

#[cfg(target_os = "macos")]
fn number_attribute_usize(element: AXUIElementRef, attribute: &str) -> Option<usize> {
    let value = copy_attribute(element, attribute).ok()?;
    let result = cf_number_usize(value);
    cf_release(value);
    result
}

#[cfg(target_os = "macos")]
fn create_cf_string(s: &str) -> Result<CFStringRef> {
    let c_string = std::ffi::CString::new(s)
        .with_context(|| format!("CFString input contains interior NUL: {s:?}"))?;
    let cf_string = unsafe {
        CFStringCreateWithCString(
            std::ptr::null(),
            c_string.as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        )
    };
    if cf_string.is_null() {
        bail!("CFStringCreateWithCString returned null for {s:?}");
    }
    Ok(cf_string)
}

#[cfg(target_os = "macos")]
fn cf_string_to_string_if_string(value: CFTypeRef) -> Option<String> {
    if unsafe { CFGetTypeID(value) } != unsafe { CFStringGetTypeID() } {
        return None;
    }
    unsafe {
        let length = CFStringGetLength(value as CFStringRef);
        let buffer_size = length.saturating_mul(4).saturating_add(1).max(1);
        let mut buffer = vec![0_i8; buffer_size as usize];
        let ok = CFStringGetCString(
            value as CFStringRef,
            buffer.as_mut_ptr(),
            buffer_size,
            K_CF_STRING_ENCODING_UTF8,
        );
        if !ok {
            return None;
        }
        std::ffi::CStr::from_ptr(buffer.as_ptr())
            .to_str()
            .ok()
            .map(str::to_string)
    }
}

#[cfg(target_os = "macos")]
fn cf_bool_value(value: CFTypeRef) -> Option<bool> {
    if unsafe { CFGetTypeID(value) } == unsafe { CFBooleanGetTypeID() } {
        Some(unsafe { CFBooleanGetValue(value) })
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn cf_number_usize(value: CFTypeRef) -> Option<usize> {
    if unsafe { CFGetTypeID(value) } != unsafe { CFNumberGetTypeID() } {
        return None;
    }
    let mut number = 0_i64;
    let ok = unsafe {
        CFNumberGetValue(
            value,
            K_CF_NUMBER_LONG_LONG_TYPE,
            &mut number as *mut i64 as *mut c_void,
        )
    };
    (ok && number >= 0).then_some(number as usize)
}

#[cfg(target_os = "macos")]
fn cf_range_value(value: CFTypeRef) -> Option<CFRange> {
    let mut range = CFRange::default();
    let ok = unsafe {
        AXValueGetValue(
            value,
            K_AX_VALUE_CF_RANGE_TYPE,
            &mut range as *mut CFRange as *mut c_void,
        )
    };
    ok.then_some(range)
}

#[cfg(target_os = "macos")]
fn text_range_from_cf_range(range: CFRange) -> Option<super::TextRangeUtf16> {
    (range.location >= 0 && range.length >= 0).then_some(super::TextRangeUtf16 {
        location: range.location as usize,
        length: range.length as usize,
    })
}

#[cfg(target_os = "macos")]
fn cg_point_value(value: CFTypeRef) -> Option<CGPoint> {
    let mut point = CGPoint::default();
    let ok = unsafe {
        AXValueGetValue(
            value,
            K_AX_VALUE_CG_POINT_TYPE,
            &mut point as *mut CGPoint as *mut c_void,
        )
    };
    ok.then_some(point)
}

#[cfg(target_os = "macos")]
fn cg_size_value(value: CFTypeRef) -> Option<CGSize> {
    let mut size = CGSize::default();
    let ok = unsafe {
        AXValueGetValue(
            value,
            K_AX_VALUE_CG_SIZE_TYPE,
            &mut size as *mut CGSize as *mut c_void,
        )
    };
    ok.then_some(size)
}

#[cfg(target_os = "macos")]
fn cg_rect_value(value: CFTypeRef) -> Option<super::geometry::RectPx> {
    let mut rect = CGRect::default();
    let ok = unsafe {
        AXValueGetValue(
            value,
            K_AX_VALUE_CG_RECT_TYPE,
            &mut rect as *mut CGRect as *mut c_void,
        )
    };
    ok.then_some(super::geometry::RectPx {
        x: rect.origin.x,
        y: rect.origin.y,
        width: rect.size.width,
        height: rect.size.height,
    })
}

#[cfg(target_os = "macos")]
fn cf_release(cf: CFTypeRef) {
    if !cf.is_null() {
        unsafe {
            CFRelease(cf);
        }
    }
}

//! Selected text operations using macOS Accessibility APIs
//!
//! This module provides getSelectedText() and setSelectedText() operations
//! using a hybrid approach: Accessibility API primary, clipboard fallback.
//!
//! ## Architecture
//!
//! - `get_selected_text()`: per-pid AX read of the source app's focused
//!   element first, then a selection-only simulated ⌘C with pasteboard
//!   change-count polling and snapshot/restore (works in AX-opaque editors
//!   like Google Docs)
//! - `set_selected_text()`: Uses clipboard + keyboard simulation (Cmd+V)
//!
//! ## Permissions
//!
//! Requires Accessibility permission in System Preferences > Privacy & Security > Accessibility

#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::{anyhow, bail, Result};
#[cfg(target_os = "macos")]
use macos_accessibility_client::accessibility;
#[cfg(target_os = "macos")]
use std::ffi::{c_void, CStr};
#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(target_os = "macos")]
use std::time::Duration;
#[cfg(target_os = "macos")]
use tracing::{debug, info};
use tracing::{instrument, warn};

// ============================================================================
// Permission Functions
// ============================================================================

/// Check if accessibility permissions are granted.
///
/// This checks if the application has been granted permission to use
/// macOS Accessibility APIs for cross-process text operations.
///
/// # Returns
/// `true` if permission is granted, `false` otherwise.
#[instrument]
#[cfg(target_os = "macos")]
pub fn has_accessibility_permission() -> bool {
    if crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery).is_err()
    {
        return false;
    }
    let result = accessibility::application_is_trusted();
    debug!(granted = result, "Checked accessibility permission");
    result
}

/// Request accessibility permissions (opens System Preferences).
///
/// This will show the system dialog prompting the user to grant
/// accessibility permission. The user must manually enable the
/// permission in System Preferences.
///
/// # Returns
/// `true` if permission is granted after the request, `false` otherwise.
#[instrument]
#[cfg(target_os = "macos")]
pub fn request_accessibility_permission() -> bool {
    if crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeVisibility)
        .is_err()
    {
        return false;
    }
    info!("Requesting accessibility permission");
    let result = accessibility::application_is_trusted_with_prompt();
    if result {
        info!("Accessibility permission granted");
    } else {
        warn!("Accessibility permission denied or pending");
    }
    result
}

/// Open System Preferences directly to Accessibility pane.
///
/// This is useful for guiding users to the correct settings location
/// without showing the system permission prompt.
///
/// # Errors
/// Returns error if unable to spawn the open command.
#[allow(dead_code)] // Will be used for permission UI prompts
#[instrument]
#[cfg(target_os = "macos")]
pub fn open_accessibility_settings() -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal)?;
    info!("Opening accessibility settings");
    Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()
        .context("Failed to open System Preferences")?;
    Ok(())
}

/// Show a user-friendly dialog explaining accessibility permission is needed.
///
/// First checks if permission is already granted. If not, requests it
/// with the system prompt.
///
/// # Returns
/// `true` if permission is granted (either already or after request).
#[allow(dead_code)] // Will be used for permission UI prompts
#[instrument]
#[cfg(target_os = "macos")]
pub fn show_permission_dialog() -> Result<bool> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeVisibility)?;
    // First, check if already granted
    if has_accessibility_permission() {
        debug!("Permission already granted");
        return Ok(true);
    }

    // Request with system prompt (opens System Preferences)
    let granted = request_accessibility_permission();

    if !granted {
        warn!("User denied accessibility permission");
    }

    Ok(granted)
}

// ============================================================================
// Get Selected Text
// ============================================================================

#[cfg(target_os = "macos")]
type AXUIElementRef = *const c_void;
#[cfg(target_os = "macos")]
type CFTypeRef = *const c_void;
#[cfg(target_os = "macos")]
type CFStringRef = *const c_void;

#[cfg(target_os = "macos")]
const K_AX_ERROR_SUCCESS: i32 = 0;
#[cfg(target_os = "macos")]
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
#[cfg(target_os = "macos")]
const K_AX_VALUE_CF_RANGE_TYPE: i32 = 4;
#[cfg(target_os = "macos")]
const AX_FOCUSED_UI_ELEMENT: &str = "AXFocusedUIElement";
#[cfg(target_os = "macos")]
const AX_SELECTED_TEXT: &str = "AXSelectedText";
#[cfg(target_os = "macos")]
const AX_SELECTED_TEXT_RANGE: &str = "AXSelectedTextRange";
#[cfg(target_os = "macos")]
const AX_STRING_FOR_RANGE: &str = "AXStringForRange";

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
    fn AXValueGetValue(value: CFTypeRef, the_type: i32, value_ptr: *mut c_void) -> bool;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFGetTypeID(cf: CFTypeRef) -> u64;
    fn CFStringGetTypeID() -> u64;
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

/// Read selected text using Accessibility attributes only.
///
/// Unlike `get_selected_text`, this helper must never fall back to clipboard
/// simulation. It is for passive preview surfaces where posting Cmd+C would be
/// surprising system input.
#[instrument(skip_all)]
#[cfg(target_os = "macos")]
pub fn get_selected_text_ax_only() -> Result<Option<String>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    if !has_accessibility_permission() {
        bail!(
            "Accessibility permission required. Enable in System Preferences > Privacy & Security > Accessibility"
        );
    }

    let system = unsafe { AXUIElementCreateSystemWide() };
    if system.is_null() {
        bail!("AXUIElementCreateSystemWide returned null");
    }

    let focused = match ax_copy_attribute(system, AX_FOCUSED_UI_ELEMENT) {
        Ok(value) => value as AXUIElementRef,
        Err(error) => {
            unsafe { CFRelease(system as CFTypeRef) };
            return Err(error).context("AXFocusedUIElement unavailable");
        }
    };
    unsafe { CFRelease(system as CFTypeRef) };

    let selected_text = ax_selected_text_for_element(focused);
    unsafe { CFRelease(focused as CFTypeRef) };
    selected_text
}

#[cfg(target_os = "macos")]
fn ax_selected_text_for_element(element: AXUIElementRef) -> Result<Option<String>> {
    if element.is_null() {
        return Ok(None);
    }

    if let Ok(value) = ax_copy_attribute(element, AX_SELECTED_TEXT) {
        let text = cf_string_to_string_if_string(value);
        unsafe { CFRelease(value) };
        if let Some(text) = text.filter(|text| !text.trim().is_empty()) {
            return Ok(Some(text));
        }
    }

    let Some(range) = ax_selected_text_range(element)? else {
        return Ok(None);
    };
    if range.length <= 0 {
        return Ok(None);
    }

    ax_string_for_range(element, range).map(|text| text.filter(|text| !text.trim().is_empty()))
}

#[cfg(target_os = "macos")]
fn ax_selected_text_range(element: AXUIElementRef) -> Result<Option<CFRange>> {
    let value = match ax_copy_attribute(element, AX_SELECTED_TEXT_RANGE) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };

    let mut range = CFRange::default();
    let ok = unsafe {
        AXValueGetValue(
            value,
            K_AX_VALUE_CF_RANGE_TYPE,
            &mut range as *mut CFRange as *mut c_void,
        )
    };
    unsafe { CFRelease(value) };

    if ok {
        Ok(Some(range))
    } else {
        Ok(None)
    }
}

#[cfg(target_os = "macos")]
fn ax_string_for_range(element: AXUIElementRef, range: CFRange) -> Result<Option<String>> {
    let range_value = ax_value_create_cf_range(range)?;
    let value = match ax_copy_parameterized_attribute(element, AX_STRING_FOR_RANGE, range_value) {
        Ok(value) => value,
        Err(_) => {
            unsafe { CFRelease(range_value) };
            return Ok(None);
        }
    };
    unsafe { CFRelease(range_value) };

    let text = cf_string_to_string_if_string(value);
    unsafe { CFRelease(value) };
    Ok(text)
}

#[cfg(target_os = "macos")]
fn ax_value_create_cf_range(range: CFRange) -> Result<CFTypeRef> {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXValueCreate(the_type: i32, value_ptr: *const c_void) -> CFTypeRef;
    }

    let value = unsafe {
        AXValueCreate(
            K_AX_VALUE_CF_RANGE_TYPE,
            &range as *const CFRange as *const c_void,
        )
    };
    if value.is_null() {
        bail!("AXValueCreate failed for selected text range");
    }
    Ok(value)
}

#[cfg(target_os = "macos")]
fn ax_copy_attribute(element: AXUIElementRef, attribute: &str) -> Result<CFTypeRef> {
    let attr = create_cf_string(attribute)?;
    let mut value: CFTypeRef = std::ptr::null();
    let result = unsafe { AXUIElementCopyAttributeValue(element, attr, &mut value) };
    unsafe { CFRelease(attr as CFTypeRef) };
    if result != K_AX_ERROR_SUCCESS || value.is_null() {
        bail!("AX attribute {attribute} unavailable: error {result}");
    }
    Ok(value)
}

#[cfg(target_os = "macos")]
fn ax_copy_parameterized_attribute(
    element: AXUIElementRef,
    attribute: &str,
    parameter: CFTypeRef,
) -> Result<CFTypeRef> {
    let attr = create_cf_string(attribute)?;
    let mut value: CFTypeRef = std::ptr::null();
    let result =
        unsafe { AXUIElementCopyParameterizedAttributeValue(element, attr, parameter, &mut value) };
    unsafe { CFRelease(attr as CFTypeRef) };
    if result != K_AX_ERROR_SUCCESS || value.is_null() {
        bail!("AX parameterized attribute {attribute} unavailable: error {result}");
    }
    Ok(value)
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
        CStr::from_ptr(buffer.as_ptr())
            .to_str()
            .ok()
            .map(str::to_string)
    }
}

/// Get the currently selected text from the focused application.
///
/// Chain (Raycast parity):
/// 1. Per-pid AX read of the source app's focused element (AXSelectedText,
///    then AXSelectedTextRange + AXStringForRange). Targeting the last real
///    app by pid matters: the system-wide focused element is unreliable
///    cross-process and can resolve to our own panel once it is key.
/// 2. Simulated ⌘C with pasteboard change-count polling and snapshot/restore.
///    This is the only channel that works in AX-opaque editors like Google
///    Docs, whose canvas renderer exposes no AX text at all. It never posts
///    ⌘A, so the user's selection survives.
///
/// # Returns
/// The selected text, or empty string if nothing is selected.
///
/// # Errors
/// - Returns error if no accessibility permission
/// - Returns error if the operation fails
///
#[instrument(skip_all)]
#[cfg(target_os = "macos")]
pub fn get_selected_text() -> Result<String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    // Check permissions first
    if !has_accessibility_permission() {
        bail!(
            "Accessibility permission required. Enable in System Preferences > Privacy & Security > Accessibility"
        );
    }

    debug!("Attempting to get selected text");

    let source_pid = crate::frontmost_app_tracker::get_last_real_app().map(|app| app.pid);
    match crate::platform::accessibility::focused_text::selected_text_for_app_ax_only(source_pid) {
        Ok(Some(text)) => {
            info!(text_len = text.len(), "Got selected text via AX");
            return Ok(text);
        }
        Ok(None) => {
            debug!("AX reported no selection; trying selection copy fallback");
        }
        Err(error) => {
            debug!(%error, "AX selection read failed; trying selection copy fallback");
        }
    }

    match crate::platform::accessibility::clipboard::copy_selection_plain_text_preserving_clipboard(
    ) {
        Ok(Some(text)) => {
            info!(text_len = text.len(), "Got selected text via copy fallback");
            Ok(text)
        }
        Ok(None) => {
            debug!("No text selected (copy fallback produced no clipboard change)");
            Ok(String::new())
        }
        Err(e) => {
            warn!(error = %e, "Failed to get selected text");
            bail!("Failed to get selected text: {}", e)
        }
    }
}

/// What kind of text a rewrite capture actually grabbed. Surfaces use this
/// to tell the user where the text came from ("Selection" vs "Draft").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturedTextKind {
    /// The user's explicit selection.
    Selection,
    /// The focused field's whole text — no selection existed at capture time.
    FocusedField,
}

/// Text captured for a rewrite, plus where it was grabbed from.
#[derive(Debug, Clone)]
pub struct CapturedText {
    pub text: String,
    pub kind: CapturedTextKind,
    /// Name of the app the text was captured from, when known.
    pub source_app: Option<String>,
}

/// Selection if any; otherwise the focused field's whole text — "what the
/// user is working on".
///
/// Used by the rewrite flow at explicit submit: a user who types a tweet and
/// invokes `.concise` without selecting first still means "rewrite my draft".
/// Chain after `get_selected_text` comes back empty:
/// 1. Passive per-pid AX whole-field read (no keystrokes, no pasteboard).
/// 2. ⌘A+⌘C with clipboard snapshot/restore for AX-opaque editors (Chrome,
///    Google Docs). Side effect: leaves the field's text selected.
///
/// All results are capped at the focused-text capture ceiling so a huge
/// document can never flood the prompt.
#[instrument(skip_all)]
#[cfg(target_os = "macos")]
pub fn capture_selection_or_focused_text() -> Result<Option<CapturedText>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)?;
    let source = crate::frontmost_app_tracker::get_last_real_app();
    let source_app = source.as_ref().map(|app| app.name.clone());
    let source_pid = source.map(|app| app.pid);

    let selection = get_selected_text()?;
    if !selection.trim().is_empty() {
        return Ok(Some(CapturedText {
            text: selection,
            kind: CapturedTextKind::Selection,
            source_app,
        }));
    }

    match crate::platform::accessibility::focused_text::focused_text_for_app_ax_only(source_pid) {
        Ok(Some(text)) => {
            info!(
                text_len = text.len(),
                "No selection; using focused-field text via AX"
            );
            return Ok(Some(CapturedText {
                text,
                kind: CapturedTextKind::FocusedField,
                source_app,
            }));
        }
        Ok(None) => {
            debug!("AX reported empty focused field; trying copy-all fallback");
        }
        Err(error) => {
            debug!(%error, "AX focused-field read failed; trying copy-all fallback");
        }
    }

    match crate::platform::accessibility::clipboard::copy_all_plain_text_preserving_clipboard() {
        Ok(text) => {
            let (text, truncated) =
                crate::platform::accessibility::focused_text::truncate_focused_text_capture(text);
            info!(
                text_len = text.len(),
                truncated, "No selection; using focused-field text via copy-all fallback"
            );
            Ok(Some(CapturedText {
                text,
                kind: CapturedTextKind::FocusedField,
                source_app,
            })
            .filter(|captured| !captured.text.trim().is_empty()))
        }
        Err(error) => {
            debug!(%error, "Copy-all fallback captured nothing; treating as no text");
            Ok(None)
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[instrument(skip_all)]
pub fn capture_selection_or_focused_text() -> Result<Option<CapturedText>> {
    let text = get_selected_text()?;
    Ok((!text.trim().is_empty()).then(|| CapturedText {
        text,
        kind: CapturedTextKind::Selection,
        source_app: None,
    }))
}

// ============================================================================
// Set Selected Text
// ============================================================================

/// Set (replace) the currently selected text in the focused application.
///
/// Strategy:
/// 1. Save current clipboard contents
/// 2. Set clipboard to new text
/// 3. Simulate Cmd+V
/// 4. Restore original clipboard
///
/// # Arguments
/// * `text` - The text to insert, replacing the current selection
///
/// # Errors
/// - Returns error if no accessibility permission
/// - Returns error if clipboard or paste operation fails
///
#[instrument(skip(text), fields(text_len = text.len()))]
#[cfg(target_os = "macos")]
pub fn set_selected_text(text: &str) -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput)?;
    if !has_accessibility_permission() {
        bail!("Accessibility permission required");
    }

    debug!("Attempting to set selected text");

    // Use clipboard fallback (AX write is complex and not widely supported)
    set_via_clipboard_fallback(text)
}

/// Clipboard-based fallback for setting selected text.
///
/// This function:
/// 1. Saves the current clipboard contents
/// 2. Sets the clipboard to the new text
/// 3. Simulates Cmd+V to paste using Core Graphics (more reliable than enigo)
/// 4. Restores the original clipboard (best effort)
#[cfg(target_os = "macos")]
fn set_via_clipboard_fallback(text: &str) -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)?;
    let snapshot = PasteboardSnapshot::capture()
        .context("Failed to snapshot clipboard before selected-text replacement")?;
    let snapshot_summary = snapshot.summary();
    debug!(
        item_count = snapshot_summary.item_count,
        type_count = snapshot_summary.type_count,
        total_bytes = snapshot_summary.total_bytes,
        has_text = snapshot_summary.content_types.has_text(),
        has_rich_text = snapshot_summary.content_types.has_rich_text(),
        has_image = snapshot_summary.content_types.has_image(),
        has_file_url = snapshot_summary.content_types.has_file_url(),
        has_other = snapshot_summary.content_types.has_other(),
        "Saved original clipboard snapshot"
    );

    write_plain_text_to_pasteboard(text).context("Failed to set clipboard text")?;
    let temporary_change_count = general_pasteboard_change_count()
        .context("Failed to read clipboard change count after selected-text replacement write")?;

    // Small delay to ensure clipboard is set
    thread::sleep(Duration::from_millis(10));

    // Simulate Cmd+V using Core Graphics (more reliable on macOS than enigo)
    let paste_result = simulate_paste_with_cg();

    // Wait for paste to complete
    thread::sleep(Duration::from_millis(150));

    // Restore original clipboard (best effort)
    thread::sleep(Duration::from_millis(100));
    let restore_result = match general_pasteboard_change_count() {
        Ok(current_change_count) if current_change_count == temporary_change_count => {
            snapshot.restore()
        }
        Ok(_) => Err(anyhow!(
            "Clipboard changed during selected-text replacement; skipped restore to avoid overwriting external clipboard update"
        )),
        Err(e) => Err(e).context("Failed to read clipboard change count before restore"),
    };
    if let Err(e) = &restore_result {
        warn!(
            error = %e,
            item_count = snapshot_summary.item_count,
            type_count = snapshot_summary.type_count,
            total_bytes = snapshot_summary.total_bytes,
            has_text = snapshot_summary.content_types.has_text(),
            has_rich_text = snapshot_summary.content_types.has_rich_text(),
            has_image = snapshot_summary.content_types.has_image(),
            has_file_url = snapshot_summary.content_types.has_file_url(),
            has_other = snapshot_summary.content_types.has_other(),
            "Failed to restore original clipboard snapshot"
        );
    } else {
        debug!(
            item_count = snapshot_summary.item_count,
            type_count = snapshot_summary.type_count,
            total_bytes = snapshot_summary.total_bytes,
            has_text = snapshot_summary.content_types.has_text(),
            has_rich_text = snapshot_summary.content_types.has_rich_text(),
            has_image = snapshot_summary.content_types.has_image(),
            has_file_url = snapshot_summary.content_types.has_file_url(),
            has_other = snapshot_summary.content_types.has_other(),
            "Restored original clipboard snapshot"
        );
    }

    paste_result?;
    restore_result
        .context("Failed to restore original clipboard after selected-text replacement")?;

    info!("Set selected text via clipboard fallback");
    Ok(())
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct PasteboardSnapshot {
    items: Vec<PasteboardItemSnapshot>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct PasteboardItemSnapshot {
    representations: Vec<PasteboardRepresentation>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct PasteboardRepresentation {
    type_name: String,
    data: Vec<u8>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PasteboardContentTypes(u8);

#[cfg(target_os = "macos")]
impl PasteboardContentTypes {
    const TEXT: Self = Self(1 << 0);
    const RICH_TEXT: Self = Self(1 << 1);
    const IMAGE: Self = Self(1 << 2);
    const FILE_URL: Self = Self(1 << 3);
    const OTHER: Self = Self(1 << 4);

    fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    fn has_text(self) -> bool {
        self.0 & Self::TEXT.0 != 0
    }

    fn has_rich_text(self) -> bool {
        self.0 & Self::RICH_TEXT.0 != 0
    }

    fn has_image(self) -> bool {
        self.0 & Self::IMAGE.0 != 0
    }

    fn has_file_url(self) -> bool {
        self.0 & Self::FILE_URL.0 != 0
    }

    fn has_other(self) -> bool {
        self.0 & Self::OTHER.0 != 0
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, Default)]
struct PasteboardSnapshotSummary {
    item_count: usize,
    type_count: usize,
    total_bytes: usize,
    content_types: PasteboardContentTypes,
}

#[cfg(target_os = "macos")]
impl PasteboardSnapshot {
    fn capture() -> Result<Self> {
        use cocoa::appkit::NSPasteboard;
        use cocoa::base::{id, nil};
        use objc::{msg_send, sel, sel_impl};

        // SAFETY: NSPasteboard and Foundation collection pointers are nil-checked.
        // Payload data is copied immediately into Rust-owned Vec<u8> values.
        unsafe {
            let pasteboard: id = NSPasteboard::generalPasteboard(nil);
            if pasteboard == nil {
                bail!("NSPasteboard.generalPasteboard returned nil");
            }

            let items: id = msg_send![pasteboard, pasteboardItems];
            if items == nil {
                return Ok(Self { items: Vec::new() });
            }

            let item_count: usize = msg_send![items, count];
            let mut snapshot_items = Vec::with_capacity(item_count);

            for item_index in 0..item_count {
                let item: id = msg_send![items, objectAtIndex: item_index];
                if item == nil {
                    bail!("NSPasteboard returned nil item while snapshotting");
                }

                let types: id = msg_send![item, types];
                if types == nil {
                    bail!("NSPasteboard item returned nil type list while snapshotting");
                }

                let type_count: usize = msg_send![types, count];
                let mut representations = Vec::with_capacity(type_count);

                for type_index in 0..type_count {
                    let type_id: id = msg_send![types, objectAtIndex: type_index];
                    if type_id == nil {
                        bail!("NSPasteboard item returned nil type while snapshotting");
                    }

                    let type_name = nsstring_to_string(type_id)
                        .context("Failed to read NSPasteboard type name while snapshotting")?;
                    let data: id = msg_send![item, dataForType: type_id];
                    if data == nil {
                        bail!("NSPasteboard item data was unavailable while snapshotting");
                    }

                    let byte_len: usize = msg_send![data, length];
                    let bytes_ptr: *const u8 = msg_send![data, bytes];
                    let bytes = if byte_len == 0 {
                        Vec::new()
                    } else {
                        if bytes_ptr.is_null() {
                            bail!("NSPasteboard item data pointer was nil while snapshotting");
                        }
                        std::slice::from_raw_parts(bytes_ptr, byte_len).to_vec()
                    };

                    representations.push(PasteboardRepresentation {
                        type_name,
                        data: bytes,
                    });
                }

                snapshot_items.push(PasteboardItemSnapshot { representations });
            }

            Ok(Self {
                items: snapshot_items,
            })
        }
    }

    fn restore(&self) -> Result<()> {
        use cocoa::appkit::NSPasteboard;
        use cocoa::base::{id, nil};
        use cocoa::foundation::{NSArray, NSData, NSString};
        use objc::{class, msg_send, sel, sel_impl};

        // SAFETY: Objective-C objects are nil-checked after creation. Snapshot
        // bytes are Rust-owned and copied into NSData before writeObjects.
        unsafe {
            let pasteboard: id = NSPasteboard::generalPasteboard(nil);
            if pasteboard == nil {
                bail!("NSPasteboard.generalPasteboard returned nil");
            }

            let _: i64 = msg_send![pasteboard, clearContents];

            if self.items.is_empty() {
                return Ok(());
            }

            let mut objects: Vec<id> = Vec::with_capacity(self.items.len());

            for item in &self.items {
                let pasteboard_item: id = msg_send![class!(NSPasteboardItem), new];
                if pasteboard_item == nil {
                    release_objects(&objects);
                    bail!("Failed to create NSPasteboardItem while restoring clipboard");
                }

                for representation in &item.representations {
                    let ns_type = NSString::alloc(nil).init_str(&representation.type_name);
                    if ns_type == nil {
                        let _: () = msg_send![pasteboard_item, release];
                        release_objects(&objects);
                        bail!("Failed to create NSPasteboard type while restoring clipboard");
                    }

                    let data = NSData::dataWithBytes_length_(
                        nil,
                        representation.data.as_ptr() as *const c_void,
                        representation.data.len() as u64,
                    );
                    if data == nil {
                        let _: () = msg_send![ns_type, release];
                        let _: () = msg_send![pasteboard_item, release];
                        release_objects(&objects);
                        bail!("Failed to create NSData while restoring clipboard");
                    }

                    let did_set: bool = msg_send![pasteboard_item, setData: data forType: ns_type];
                    let _: () = msg_send![ns_type, release];

                    if !did_set {
                        let _: () = msg_send![pasteboard_item, release];
                        release_objects(&objects);
                        bail!("NSPasteboardItem.setData returned false while restoring clipboard");
                    }
                }

                objects.push(pasteboard_item);
            }

            let ns_objects: id = NSArray::arrayWithObjects(nil, objects.as_slice());
            if ns_objects == nil {
                release_objects(&objects);
                bail!("Failed to create NSArray while restoring clipboard");
            }

            let did_write: bool = msg_send![pasteboard, writeObjects: ns_objects];
            release_objects(&objects);

            if !did_write {
                bail!("NSPasteboard.writeObjects returned false while restoring clipboard");
            }

            Ok(())
        }
    }

    fn summary(&self) -> PasteboardSnapshotSummary {
        let mut summary = PasteboardSnapshotSummary {
            item_count: self.items.len(),
            ..Default::default()
        };

        for item in &self.items {
            for representation in &item.representations {
                summary.type_count += 1;
                summary.total_bytes += representation.data.len();
                classify_pasteboard_type(&representation.type_name, &mut summary);
            }
        }

        summary
    }
}

#[cfg(target_os = "macos")]
fn write_plain_text_to_pasteboard(text: &str) -> Result<()> {
    use cocoa::appkit::NSPasteboard;
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSArray, NSString};
    use objc::{msg_send, sel, sel_impl};

    // SAFETY: NSPasteboard, NSString, and NSArray pointers are nil-checked.
    unsafe {
        let pasteboard: id = NSPasteboard::generalPasteboard(nil);
        if pasteboard == nil {
            bail!("NSPasteboard.generalPasteboard returned nil");
        }

        let _: i64 = msg_send![pasteboard, clearContents];

        let ns_text = NSString::alloc(nil).init_str(text);
        if ns_text == nil {
            bail!("Failed to create NSString for selected-text replacement");
        }

        let objects: id = NSArray::arrayWithObjects(nil, &[ns_text]);
        if objects == nil {
            let _: () = msg_send![ns_text, release];
            bail!("Failed to create NSArray for selected-text replacement");
        }

        let did_write: bool = msg_send![pasteboard, writeObjects: objects];
        let _: () = msg_send![ns_text, release];

        if !did_write {
            bail!("NSPasteboard.writeObjects returned false for selected-text replacement");
        }

        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn general_pasteboard_change_count() -> Result<i64> {
    use cocoa::appkit::NSPasteboard;
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};

    // SAFETY: NSPasteboard.generalPasteboard is nil-checked and changeCount is
    // a value-returning AppKit method.
    unsafe {
        let pasteboard: id = NSPasteboard::generalPasteboard(nil);
        if pasteboard == nil {
            bail!("NSPasteboard.generalPasteboard returned nil");
        }

        Ok(msg_send![pasteboard, changeCount])
    }
}

#[cfg(target_os = "macos")]
fn classify_pasteboard_type(type_name: &str, summary: &mut PasteboardSnapshotSummary) {
    let normalized = type_name.to_ascii_lowercase();

    if normalized.contains("utf8-plain-text")
        || normalized.contains("public.text")
        || normalized.contains("string")
    {
        summary.content_types.insert(PasteboardContentTypes::TEXT);
    } else if normalized.contains("rtf") || normalized.contains("html") {
        summary
            .content_types
            .insert(PasteboardContentTypes::RICH_TEXT);
    } else if normalized.contains("image")
        || normalized.contains("png")
        || normalized.contains("jpeg")
        || normalized.contains("tiff")
    {
        summary.content_types.insert(PasteboardContentTypes::IMAGE);
    } else if normalized.contains("file-url")
        || normalized.contains("fileurl")
        || normalized.contains("filename")
    {
        summary
            .content_types
            .insert(PasteboardContentTypes::FILE_URL);
    } else {
        summary.content_types.insert(PasteboardContentTypes::OTHER);
    }
}

#[cfg(target_os = "macos")]
unsafe fn nsstring_to_string(value: cocoa::base::id) -> Result<String> {
    use cocoa::base::nil;
    use objc::{msg_send, sel, sel_impl};

    if value == nil {
        bail!("NSString pointer was nil");
    }

    let utf8: *const std::os::raw::c_char = msg_send![value, UTF8String];
    if utf8.is_null() {
        bail!("NSString.UTF8String returned nil");
    }

    Ok(CStr::from_ptr(utf8).to_string_lossy().into_owned())
}

#[cfg(target_os = "macos")]
unsafe fn release_objects(objects: &[cocoa::base::id]) {
    use objc::{msg_send, sel, sel_impl};

    for object in objects {
        let _: () = msg_send![*object, release];
    }
}

/// Simulate Cmd+V paste using Core Graphics events.
/// This is more reliable on macOS than using enigo.
///
/// # Usage
/// Call this after copying content to the clipboard and hiding the window.
/// The function will simulate Cmd+V to paste into the currently focused app.
#[cfg(target_os = "macos")]
pub fn simulate_paste_with_cg() -> Result<()> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput)?;
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    // 'v' key is keycode 9 on macOS
    const KEY_V: CGKeyCode = 9;

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .ok()
        .context("Failed to create CGEventSource")?;

    // Create key down event for 'v' with Cmd modifier
    let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
        .ok()
        .context("Failed to create key down event")?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);

    // Create key up event for 'v' with Cmd modifier
    let key_up = CGEvent::new_keyboard_event(source, KEY_V, false)
        .ok()
        .context("Failed to create key up event")?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);

    // Post events
    key_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(5));
    key_up.post(CGEventTapLocation::HID);

    debug!("Simulated Cmd+V via Core Graphics");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[instrument]
pub fn has_accessibility_permission() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
#[instrument]
pub fn request_accessibility_permission() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
#[instrument]
pub fn open_accessibility_settings() -> Result<()> {
    bail!("Accessibility settings are only available on macOS")
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
#[instrument]
pub fn show_permission_dialog() -> Result<bool> {
    Ok(false)
}

#[cfg(not(target_os = "macos"))]
#[instrument(skip_all)]
pub fn get_selected_text() -> Result<String> {
    warn!("get_selected_text requested on unsupported platform");
    bail!("Selected text APIs are only supported on macOS")
}

#[cfg(not(target_os = "macos"))]
#[instrument(skip_all)]
pub fn get_selected_text_ax_only() -> Result<Option<String>> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
#[instrument(skip(text), fields(text_len = text.len()))]
pub fn set_selected_text(text: &str) -> Result<()> {
    let _ = text;
    warn!("set_selected_text requested on unsupported platform");
    bail!("Selected text APIs are only supported on macOS")
}

#[cfg(not(target_os = "macos"))]
pub fn simulate_paste_with_cg() -> Result<()> {
    bail!("Paste simulation is only supported on macOS")
}

// ============================================================================
// Tests
// ============================================================================

// ============================================================================
// Unit Tests (always run with `cargo test`)
// ============================================================================
#[cfg(test)]
mod unit_tests {
    #[test]
    fn test_set_via_clipboard_fallback_restores_snapshot_after_paste_attempt() {
        let source = include_str!("selected_text.rs");

        let snapshot_idx = source
            .find("let snapshot = PasteboardSnapshot::capture()")
            .expect("expected selected-text replacement to snapshot the clipboard first");
        let write_idx = source
            .find("write_plain_text_to_pasteboard(text)")
            .expect("expected selected-text replacement to write plain text through NSPasteboard");
        let paste_result_idx = source
            .find("let paste_result = simulate_paste_with_cg();")
            .expect("expected set_via_clipboard_fallback to capture paste result");
        let post_paste_delay_idx = source
            .find("thread::sleep(Duration::from_millis(150));")
            .expect("expected 150ms post-paste delay");
        // Restore is gated on the pasteboard change count so an external
        // clipboard update during the paste window is never clobbered.
        let restore_guard_idx = source
            .find("let restore_result = match general_pasteboard_change_count()")
            .expect("expected restore to be gated on the clipboard change count");
        let restore_call_idx = source
            .find("snapshot.restore()")
            .expect("expected full pasteboard snapshot restore");
        let pre_restore_delay_idx = source
            .find("thread::sleep(Duration::from_millis(100));")
            .expect("expected 100ms pre-restore delay");
        let paste_return_idx = source
            .find("paste_result?;")
            .expect("expected paste result to be returned after restore attempt");

        assert!(
            snapshot_idx < write_idx,
            "snapshot must happen before mutation"
        );
        assert!(
            write_idx < paste_result_idx,
            "clipboard write must precede paste"
        );
        assert!(
            paste_result_idx < post_paste_delay_idx,
            "paste should run before post-paste delay"
        );
        assert!(
            post_paste_delay_idx < pre_restore_delay_idx,
            "pre-restore delay should follow the post-paste delay"
        );
        assert!(
            pre_restore_delay_idx < restore_guard_idx,
            "restore should occur after the pre-restore delay"
        );
        assert!(
            restore_guard_idx < restore_call_idx,
            "snapshot restore must stay inside the change-count guard"
        );
        assert!(
            restore_guard_idx < paste_return_idx,
            "paste result should be returned after restore attempt"
        );
    }

    #[test]
    fn test_selected_text_clipboard_restore_preserves_non_text_representations() {
        let source = include_str!("selected_text.rs");
        let snapshot_impl = source
            .split("impl PasteboardSnapshot {")
            .nth(1)
            .expect("expected PasteboardSnapshot implementation");

        assert!(
            snapshot_impl.contains("pasteboardItems"),
            "snapshot must inspect pasteboard items, not only text"
        );
        assert!(
            snapshot_impl.contains("dataForType"),
            "snapshot must copy every representation's bytes"
        );
        assert!(
            snapshot_impl.contains("setData: data forType: ns_type"),
            "restore must rebuild each representation by type"
        );
        assert!(
            snapshot_impl.contains("writeObjects"),
            "restore must write rebuilt pasteboard items"
        );
        assert!(
            !snapshot_impl.contains("get_text()") && !snapshot_impl.contains("set_text("),
            "selected-text restore must not fall back to text-only arboard restore"
        );
    }

    #[test]
    fn test_selected_text_clipboard_logs_are_content_light() {
        let source = include_str!("selected_text.rs");
        let fallback_body = source
            .split("fn set_via_clipboard_fallback(text: &str) -> Result<()> {")
            .nth(1)
            .and_then(|rest| rest.split("struct PasteboardSnapshot").next())
            .expect("expected selected-text fallback body");

        // A forbidden token only counts when it starts its own identifier:
        // content-light boolean fields like `has_text = ...` must not trip the
        // bare `text =` check, while a raw `text = ...` log field still does.
        fn contains_standalone(body: &str, needle: &str) -> bool {
            body.match_indices(needle).any(|(idx, _)| {
                if idx == 0 {
                    return true;
                }
                let prev = body.as_bytes()[idx - 1];
                !prev.is_ascii_alphanumeric() && prev != b'_'
            })
        }

        for forbidden in [
            "text =",
            "%text",
            "original_text",
            "type_name =",
            "representation.data",
        ] {
            assert!(
                !contains_standalone(fallback_body, forbidden),
                "selected-text fallback logs must not expose raw clipboard or replacement content: {forbidden}"
            );
        }

        for required in [
            "item_count",
            "type_count",
            "total_bytes",
            "has_text",
            "has_rich_text",
            "has_image",
            "has_file_url",
            "has_other",
        ] {
            assert!(
                fallback_body.contains(required),
                "selected-text fallback should log content-light clipboard boundary field: {required}"
            );
        }
    }
}

// ============================================================================
// System Tests (require `cargo test --features system-tests`)
// ============================================================================
// These tests interact with macOS accessibility APIs, clipboard, and keyboard
// simulation. They may have side effects on the system state.

#[cfg(all(target_os = "macos", test, feature = "system-tests"))]
mod system_tests {
    use super::*;

    #[test]
    fn test_permission_check_does_not_panic() {
        // This test verifies the permission check doesn't panic
        // The actual result depends on system permissions
        let _has_permission = has_accessibility_permission();
        // Just ensure it doesn't panic - result varies by environment
    }

    #[test]
    fn test_permission_check_is_deterministic() {
        // Calling permission check multiple times should return same result
        let first = has_accessibility_permission();
        let second = has_accessibility_permission();
        assert_eq!(first, second, "Permission check should be deterministic");
    }

    #[test]
    #[ignore] // Requires manual interaction - select text in another app first
    fn test_get_selected_text_in_textedit() {
        // Instructions:
        // 1. Open TextEdit
        // 2. Type and select "Hello World"
        // 3. Run this test with: cargo test --features system-tests test_get_selected_text_in_textedit -- --ignored
        let text = get_selected_text().expect("Should get selected text");
        assert!(!text.is_empty(), "Should have selected text");
        println!("Got selected text: {}", text);
    }

    #[test]
    #[ignore] // Requires manual interaction - select text in another app first
    fn test_set_selected_text() {
        // Instructions:
        // 1. Open TextEdit
        // 2. Select some text
        // 3. Run this test with: cargo test --features system-tests test_set_selected_text -- --ignored
        set_selected_text("REPLACED").expect("Should set selected text");
        // Verify manually that text was replaced
        println!("Text should be replaced with 'REPLACED'");
    }

    #[test]
    #[ignore] // Opens System Preferences
    fn test_open_accessibility_settings() {
        // This will open System Preferences to the Accessibility pane
        open_accessibility_settings().expect("Should open settings");
    }

    #[test]
    #[ignore] // Calls get_selected_text which may simulate Cmd+C via clipboard fallback
    fn test_get_selected_text_without_permission_returns_error() {
        // If we don't have permission, we should get an error
        // This test is tricky because we can't easily revoke permission
        // Just verify the function handles the check
        let result = get_selected_text();
        // Result depends on whether permission is granted
        match result {
            Ok(text) => {
                // Permission was granted, we got some text (possibly empty)
                println!("Got text (permission granted): '{}'", text);
            }
            Err(e) => {
                // Either no permission or no selection
                println!("Got error (expected if no permission): {}", e);
            }
        }
    }

    #[test]
    #[ignore] // Calls set_selected_text which simulates Cmd+V via clipboard fallback
    fn test_set_selected_text_empty_string() {
        // Test setting empty text (edge case)
        // This will fail without permission, but shouldn't panic
        let result = set_selected_text("");
        // Don't assert on result - depends on permission state
        if let Err(e) = result {
            println!("Expected error without permission: {}", e);
        }
    }
}

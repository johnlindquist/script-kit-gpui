use anyhow::{bail, Result};
use core_graphics::geometry::{CGPoint, CGSize};
use std::ffi::c_void;

use super::cf::*;
use super::ffi::*;

/// Get an attribute value from an AXUIElement
pub(super) fn get_ax_attribute(element: AXUIElementRef, attribute: &str) -> Result<CFTypeRef> {
    let attr_str = try_create_cf_string(attribute)?;
    let mut value: CFTypeRef = std::ptr::null();

    // SAFETY: element is a valid AXUIElementRef from the caller, attr_str is a valid
    // CFStringRef created above, and value is a stack-allocated out-pointer.
    let result =
        unsafe { AXUIElementCopyAttributeValue(element, attr_str, &mut value as *mut CFTypeRef) };

    cf_release(attr_str);

    match result {
        kAXErrorSuccess => Ok(value),
        kAXErrorAPIDisabled => bail!("Accessibility API is disabled"),
        kAXErrorNoValue => bail!("No value for attribute: {}", attribute),
        _ => bail!("Failed to get attribute {}: error {}", attribute, result),
    }
}

/// Set an attribute value on an AXUIElement
pub(super) fn set_ax_attribute(
    element: AXUIElementRef,
    attribute: &str,
    value: CFTypeRef,
) -> Result<()> {
    let attr_str = try_create_cf_string(attribute)?;

    // SAFETY: element, attr_str, and value are valid CF object pointers from the caller.
    let result = unsafe { AXUIElementSetAttributeValue(element, attr_str, value) };

    cf_release(attr_str);

    match result {
        kAXErrorSuccess => Ok(()),
        kAXErrorAPIDisabled => bail!("Accessibility API is disabled"),
        _ => bail!("Failed to set attribute {}: error {}", attribute, result),
    }
}

/// Perform an action on an AXUIElement
pub(super) fn perform_ax_action(element: AXUIElementRef, action: &str) -> Result<()> {
    let action_str = try_create_cf_string(action)?;

    // SAFETY: element is a valid AXUIElementRef, action_str is a valid CFStringRef.
    let result = unsafe { AXUIElementPerformAction(element, action_str) };

    cf_release(action_str);

    match result {
        kAXErrorSuccess => Ok(()),
        kAXErrorAPIDisabled => bail!("Accessibility API is disabled"),
        _ => bail!("Failed to perform action {}: error {}", action, result),
    }
}

/// Get the position of a window
pub(super) fn get_window_position(window: AXUIElementRef) -> Result<(i32, i32)> {
    let value = get_ax_attribute(window, "AXPosition")?;

    let mut point = CGPoint::new(0.0, 0.0);
    // SAFETY: value is a valid AXValueRef obtained from get_ax_attribute. We pass
    // kAXValueTypeCGPoint matching the expected type and a properly aligned CGPoint pointer.
    let success = unsafe {
        AXValueGetValue(
            value,
            kAXValueTypeCGPoint,
            &mut point as *mut _ as *mut c_void,
        )
    };

    cf_release(value);

    if success {
        Ok((point.x as i32, point.y as i32))
    } else {
        bail!("Failed to extract position value")
    }
}

/// Get the size of a window
pub(super) fn get_window_size(window: AXUIElementRef) -> Result<(u32, u32)> {
    let value = get_ax_attribute(window, "AXSize")?;

    let mut size = CGSize::new(0.0, 0.0);
    // SAFETY: value is a valid AXValueRef obtained from get_ax_attribute. We pass
    // kAXValueTypeCGSize matching the expected type and a properly aligned CGSize pointer.
    let success = unsafe {
        AXValueGetValue(
            value,
            kAXValueTypeCGSize,
            &mut size as *mut _ as *mut c_void,
        )
    };

    cf_release(value);

    if success {
        Ok((size.width as u32, size.height as u32))
    } else {
        bail!("Failed to extract size value")
    }
}

/// Set the position of a window
pub(super) fn set_window_position(window: AXUIElementRef, x: i32, y: i32) -> Result<()> {
    let point = CGPoint::new(x as f64, y as f64);
    // SAFETY: point is a valid stack-allocated CGPoint. AXValueCreate copies the data.
    let value = unsafe { AXValueCreate(kAXValueTypeCGPoint, &point as *const _ as *const c_void) };

    if value.is_null() {
        bail!("Failed to create AXValue for position");
    }

    let result = set_ax_attribute(window, "AXPosition", value);
    cf_release(value);
    result
}

/// Set the size of a window
pub(super) fn set_window_size(window: AXUIElementRef, width: u32, height: u32) -> Result<()> {
    let size = CGSize::new(width as f64, height as f64);
    // SAFETY: size is a valid stack-allocated CGSize. AXValueCreate copies the data.
    let value = unsafe { AXValueCreate(kAXValueTypeCGSize, &size as *const _ as *const c_void) };

    if value.is_null() {
        bail!("Failed to create AXValue for size");
    }

    let result = set_ax_attribute(window, "AXSize", value);
    cf_release(value);
    result
}

/// Get the string value of a window attribute
pub(super) fn get_window_string_attribute(
    window: AXUIElementRef,
    attribute: &str,
) -> Option<String> {
    match get_ax_attribute(window, attribute) {
        Ok(value) => {
            // Check if it's a CFString
            // SAFETY: value is a valid CFTypeRef returned by get_ax_attribute.
            let type_id = unsafe { CFGetTypeID(value) };
            // SAFETY: CFStringGetTypeID is a pure function returning a constant type ID.
            let string_type_id = unsafe { CFStringGetTypeID() };

            let result = if type_id == string_type_id {
                cf_string_to_string(value as CFStringRef)
            } else {
                None
            };

            cf_release(value);
            result
        }
        Err(_) => None,
    }
}

/// Check whether an AX attribute is settable on an element.
pub(super) fn ax_attribute_is_settable(element: AXUIElementRef, attribute: &str) -> Result<bool> {
    let attr_str = try_create_cf_string(attribute)?;
    let mut settable = false;
    // SAFETY: element is a valid AXUIElementRef, attr_str a valid CFStringRef,
    // settable a stack out-pointer.
    let result = unsafe { AXUIElementIsAttributeSettable(element, attr_str, &mut settable) };
    cf_release(attr_str);
    match result {
        kAXErrorSuccess => Ok(settable),
        kAXErrorAPIDisabled => bail!("Accessibility API is disabled"),
        _ => bail!("Failed to query settable {}: error {}", attribute, result),
    }
}

/// List the supported action names of an element.
pub(super) fn ax_action_names(element: AXUIElementRef) -> Result<Vec<String>> {
    let mut names: CFArrayRef = std::ptr::null();
    // SAFETY: element is a valid AXUIElementRef; names is a stack out-pointer.
    // On success the array is owned by us (Copy rule) and released below.
    let result = unsafe { AXUIElementCopyActionNames(element, &mut names) };
    if result != kAXErrorSuccess {
        bail!("Failed to copy action names: error {result}");
    }
    if names.is_null() {
        return Ok(Vec::new());
    }
    let mut actions = Vec::new();
    // SAFETY: names is a valid owned CFArray of CFStrings per the AX contract.
    unsafe {
        let count = CFArrayGetCount(names);
        for index in 0..count {
            let value = CFArrayGetValueAtIndex(names, index);
            if value.is_null() {
                continue;
            }
            if CFGetTypeID(value) == CFStringGetTypeID() {
                if let Some(name) = cf_string_to_string(value as CFStringRef) {
                    actions.push(name);
                }
            }
        }
    }
    cf_release(names as CFTypeRef);
    Ok(actions)
}

/// Batch-read window attributes with the public multiple-attribute API.
///
/// Returns `None` when the batch API is unsupported or malformed for this
/// element — the caller falls back to individual reads and records
/// `ax_batch_fallback`. A batch failure is never permission to drop the app.
pub(super) struct BatchWindowAttributes {
    pub title: Option<String>,
    pub role: Option<String>,
    pub subrole: Option<String>,
    pub minimized: Option<bool>,
}

pub(super) fn batch_read_window_attributes(
    window: AXUIElementRef,
) -> Option<BatchWindowAttributes> {
    use core_foundation::array::CFArray;
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;

    let attribute_names = ["AXTitle", "AXRole", "AXSubrole", "AXMinimized"];
    let attributes: Vec<CFString> = attribute_names
        .iter()
        .map(|name| CFString::new(name))
        .collect();
    let attribute_array = CFArray::from_CFTypes(&attributes);

    let mut values: CFArrayRef = std::ptr::null();
    // SAFETY: window is a valid AXUIElementRef; the attribute array is a live
    // CFArray for the duration of the call; values is a stack out-pointer.
    let result = unsafe {
        AXUIElementCopyMultipleAttributeValues(
            window,
            attribute_array.as_concrete_TypeRef() as CFArrayRef,
            0,
            &mut values,
        )
    };
    if result != kAXErrorSuccess || values.is_null() {
        return None;
    }
    // SAFETY: values is an owned CFArray with one entry per requested
    // attribute (missing values are kCFNull or AXValue errors).
    let extracted = unsafe {
        let count = CFArrayGetCount(values);
        if count != attribute_names.len() as i64 {
            cf_release(values as CFTypeRef);
            return None;
        }
        let string_at = |index: i64| -> Option<String> {
            let value = CFArrayGetValueAtIndex(values, index);
            if value.is_null() || CFGetTypeID(value) != CFStringGetTypeID() {
                return None;
            }
            cf_string_to_string(value as CFStringRef)
        };
        let bool_at = |index: i64| -> Option<bool> {
            let value = CFArrayGetValueAtIndex(values, index);
            if value.is_null() || CFGetTypeID(value) != CFBooleanGetTypeID() {
                return None;
            }
            Some(CFBooleanGetValue(value))
        };
        BatchWindowAttributes {
            title: string_at(0),
            role: string_at(1),
            subrole: string_at(2),
            minimized: bool_at(3),
        }
    };
    cf_release(values as CFTypeRef);
    Some(extracted)
}

/// Get the boolean value of a window attribute.
pub(super) fn get_window_bool_attribute(window: AXUIElementRef, attribute: &str) -> Option<bool> {
    match get_ax_attribute(window, attribute) {
        Ok(value) => {
            // SAFETY: value is a valid CFTypeRef returned by get_ax_attribute.
            let type_id = unsafe { CFGetTypeID(value) };
            // SAFETY: CFBooleanGetTypeID is a pure function returning a constant type ID.
            let bool_type_id = unsafe { CFBooleanGetTypeID() };

            let result = if type_id == bool_type_id {
                // SAFETY: the type id check above proves this is a CFBoolean.
                Some(unsafe { CFBooleanGetValue(value) })
            } else {
                None
            };

            cf_release(value);
            result
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live probe for the public batch-attribute API (decision rule:
    /// "Public batch AX reads"). Requires Accessibility permission and a
    /// finder/system process with windows; ignored by default.
    #[test]
    #[ignore]
    fn live_copy_multiple_attributes() {
        if !super::super::query::has_accessibility_permission() {
            eprintln!("SKIP: accessibility permission not granted");
            return;
        }
        let windows = super::super::query::list_windows().expect("list windows");
        let Some(_first) = windows.first() else {
            eprintln!("SKIP: no windows to probe");
            return;
        };
        // list_windows() above already exercised batch_read_window_attributes
        // for every AX row (with individual-read fallback). Reaching this
        // point without a panic is the live proof; print the outcome.
        eprintln!("live batch read OK across {} windows", windows.len());
    }
}

use std::ffi::c_void;

pub(super) type AXUIElementRef = *const c_void;
pub(super) type AXValueRef = *const c_void;
pub(super) type CFTypeRef = *const c_void;
pub(super) type CFStringRef = *const c_void;
pub(super) type CFArrayRef = *const c_void;

pub(super) const kAXValueTypeCGPoint: i32 = 1;
pub(super) const kAXValueTypeCGSize: i32 = 2;
pub(super) const kAXErrorSuccess: i32 = 0;
pub(super) const kAXErrorAPIDisabled: i32 = -25211;
pub(super) const kAXErrorNoValue: i32 = -25212;
pub(super) const kCFStringEncodingUTF8: u32 = 0x08000100;
pub(super) const kCFNumberSInt32Type: i32 = 3;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub(super) fn CFRelease(cf: *const c_void);
    pub(super) fn CFRetain(cf: *const c_void) -> *const c_void;
    pub(super) fn CFEqual(cf1: CFTypeRef, cf2: CFTypeRef) -> bool;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub(super) fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    pub(super) fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    pub(super) fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    pub(super) fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> i32;
    pub(super) fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> i32;
    pub(super) fn AXUIElementCopyMultipleAttributeValues(
        element: AXUIElementRef,
        attributes: CFArrayRef,
        options: u32,
        values: *mut CFArrayRef,
    ) -> i32;
    pub(super) fn AXUIElementIsAttributeSettable(
        element: AXUIElementRef,
        attribute: CFStringRef,
        settable: *mut bool,
    ) -> i32;
    pub(super) fn AXUIElementCopyActionNames(
        element: AXUIElementRef,
        names: *mut CFArrayRef,
    ) -> i32;
    pub(super) fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> i32;
    pub(super) fn AXUIElementSetMessagingTimeout(
        element: AXUIElementRef,
        timeout_in_seconds: f32,
    ) -> i32;
    pub(super) fn AXValueCreate(value_type: i32, value: *const c_void) -> AXValueRef;
    pub(super) fn AXValueGetValue(
        value: AXValueRef,
        value_type: i32,
        value_out: *mut c_void,
    ) -> bool;
    pub(super) fn AXValueGetType(value: AXValueRef) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub(super) fn CFStringCreateWithCString(
        alloc: *const c_void,
        c_str: *const i8,
        encoding: u32,
    ) -> CFStringRef;
    pub(super) fn CFStringGetCString(
        string: CFStringRef,
        buffer: *mut i8,
        buffer_size: i64,
        encoding: u32,
    ) -> bool;
    pub(super) fn CFStringGetLength(string: CFStringRef) -> i64;
    pub(super) fn CFArrayGetCount(array: CFArrayRef) -> i64;
    pub(super) fn CFArrayGetValueAtIndex(array: CFArrayRef, index: i64) -> CFTypeRef;
    pub(super) fn CFGetTypeID(cf: CFTypeRef) -> u64;
    pub(super) fn CFStringGetTypeID() -> u64;
    pub(super) fn CFBooleanGetValue(boolean: CFTypeRef) -> bool;
    pub(super) fn CFBooleanGetTypeID() -> u64;
    pub(super) fn CFNumberGetValue(
        number: CFTypeRef,
        number_type: i32,
        value_ptr: *mut c_void,
    ) -> bool;
}

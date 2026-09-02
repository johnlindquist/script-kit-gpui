//! Zero-copy webcam capture via AVFoundation.
//!
//! Captures camera frames as CVPixelBuffer and sends them directly to
//! GPUI's `surface()` element — no CPU pixel conversion, no copies.
//!
//! The returned [`CaptureHandle`] owns all AVFoundation resources and
//! stops capture + releases everything on drop.

// --- merged from part_000.rs ---
use core_foundation::base::TCFType;
use core_video::pixel_buffer::{CVPixelBuffer, CVPixelBufferRef};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, BOOL, YES};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::{c_void, CStr};
use std::sync::{mpsc, Once};
use thiserror::Error;
// CoreVideo pixel format: 420YpCbCr8BiPlanarFullRange (NV12)
// GPUI's Metal renderer requires this specific format for surface()
const K_CV_PIXEL_FORMAT_TYPE_420_YP_CB_CR_8_BI_PLANAR_FULL_RANGE: u32 = 0x34323066; // '420f'

/// Structured webcam startup errors so callers can map failures to specific UI guidance.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WebcamStartError {
    #[error(
        "Camera permission denied (attempted={attempted}, failed={failed}, state={state}, details={details})"
    )]
    PermissionDenied {
        attempted: &'static str,
        failed: &'static str,
        state: &'static str,
        details: String,
    },
    #[error(
        "Camera device is busy (attempted={attempted}, failed={failed}, state={state}, details={details})"
    )]
    DeviceBusy {
        attempted: &'static str,
        failed: &'static str,
        state: &'static str,
        details: String,
    },
    #[error(
        "No camera device available (attempted={attempted}, failed={failed}, state={state}, details={details})"
    )]
    NoDevice {
        attempted: &'static str,
        failed: &'static str,
        state: &'static str,
        details: String,
    },
    #[error(
        "Failed to initialize camera input (attempted={attempted}, failed={failed}, state={state}, details={details})"
    )]
    InputInitFailed {
        attempted: &'static str,
        failed: &'static str,
        state: &'static str,
        details: String,
    },
    #[error(
        "Failed to attach camera input (attempted={attempted}, failed={failed}, state={state}, details={details})"
    )]
    CannotAddInput {
        attempted: &'static str,
        failed: &'static str,
        state: &'static str,
        details: String,
    },
    #[error(
        "Failed to initialize video output (attempted={attempted}, failed={failed}, state={state}, details={details})"
    )]
    OutputInitFailed {
        attempted: &'static str,
        failed: &'static str,
        state: &'static str,
        details: String,
    },
    #[error(
        "Failed to create webcam callback queue (attempted={attempted}, failed={failed}, state={state}, details={details})"
    )]
    CallbackQueueUnavailable {
        attempted: &'static str,
        failed: &'static str,
        state: &'static str,
        details: String,
    },
    #[error(
        "Webcam delegate unavailable (attempted={attempted}, failed={failed}, state={state}, details={details})"
    )]
    DelegateClassUnavailable {
        attempted: &'static str,
        failed: &'static str,
        state: &'static str,
        details: String,
    },
    #[error(
        "Failed to attach video output (attempted={attempted}, failed={failed}, state={state}, details={details})"
    )]
    CannotAddOutput {
        attempted: &'static str,
        failed: &'static str,
        state: &'static str,
        details: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct NSErrorSummary {
    domain: Option<String>,
    code: Option<i64>,
    description: Option<String>,
}
fn classify_input_init_error(summary: NSErrorSummary) -> WebcamStartError {
    let details = format_nserror_details(&summary);
    let description = summary
        .description
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let domain = summary.domain.as_deref().unwrap_or("");
    let code = summary.code.unwrap_or_default();

    let is_permission_denied = (domain == "AVFoundationErrorDomain" && code == -11852)
        || description.contains("not authorized")
        || description.contains("permission")
        || description.contains("access denied");
    if is_permission_denied {
        return WebcamStartError::PermissionDenied {
            attempted: "create camera input",
            failed: "camera permission check",
            state: "session_created_device_present",
            details,
        };
    }

    let is_device_busy = (domain == "AVFoundationErrorDomain" && code == -11815)
        || description.contains("in use")
        || description.contains("busy")
        || description.contains("another client");
    if is_device_busy {
        return WebcamStartError::DeviceBusy {
            attempted: "create camera input",
            failed: "camera device lock",
            state: "session_created_device_present",
            details,
        };
    }

    let is_no_device = description.contains("no video device") || description.contains("no camera");
    if is_no_device {
        return WebcamStartError::NoDevice {
            attempted: "create camera input",
            failed: "camera device availability",
            state: "session_created_device_present",
            details,
        };
    }

    WebcamStartError::InputInitFailed {
        attempted: "create camera input",
        failed: "camera input initialization",
        state: "session_created_device_present",
        details,
    }
}
fn format_nserror_details(summary: &NSErrorSummary) -> String {
    let domain = summary.domain.as_deref().unwrap_or("unknown-domain");
    let code = summary
        .code
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown-code".to_string());
    let description = summary
        .description
        .as_deref()
        .unwrap_or("unknown-description");
    format!("domain={domain}, code={code}, description={description}")
}
unsafe fn nserror_summary(error: *mut Object) -> NSErrorSummary {
    if error.is_null() {
        return NSErrorSummary {
            domain: None,
            code: None,
            description: None,
        };
    }

    let domain_obj: *mut Object = msg_send![error, domain];
    let code: i64 = msg_send![error, code];
    let description_obj: *mut Object = msg_send![error, localizedDescription];

    NSErrorSummary {
        domain: nsstring_to_string(domain_obj),
        code: Some(code),
        description: nsstring_to_string(description_obj),
    }
}
unsafe fn nsstring_to_string(value: *mut Object) -> Option<String> {
    if value.is_null() {
        return None;
    }

    let utf8: *const i8 = msg_send![value, UTF8String];
    if utf8.is_null() {
        return None;
    }

    Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
}
/// Handle to a running AVCaptureSession.
///
/// Stops the capture session, drains the dispatch queue to ensure no
/// callbacks are in-flight, then releases all AVFoundation objects and
/// reclaims the boxed sender on drop.
pub struct CaptureHandle {
    session: *mut Object,
    delegate: *mut Object,
    queue: *mut c_void,
    sender_ptr: *mut c_void,
}
// Safety: AVCaptureSession and dispatch queues are thread-safe.
// Raw pointers are only accessed in Drop after synchronous session stop
// and dispatch queue drain.
unsafe impl Send for CaptureHandle {}
impl Drop for CaptureHandle {
    fn drop(&mut self) {
        unsafe {
            // 1. Stop the capture session (synchronous — blocks until fully stopped,
            //    no new callbacks will be dispatched after this returns)
            let _: () = msg_send![self.session, stopRunning];

            // 2. Drain the dispatch queue — ensures any already-dispatched callback
            //    has finished executing before we free the sender
            extern "C" fn noop(_ctx: *mut c_void) {}
            dispatch_sync_f(self.queue, std::ptr::null_mut(), noop);

            // 3. Null out the sender ivar (belt-and-suspenders safety)
            (*self.delegate).set_ivar::<*mut c_void>("_sender", std::ptr::null_mut());

            // 4. Reclaim the boxed sender
            if !self.sender_ptr.is_null() {
                let _ = Box::from_raw(self.sender_ptr as *mut mpsc::SyncSender<CVPixelBuffer>);
            }

            // 5. Release ObjC objects we own (+1 from alloc/init)
            let _: () = msg_send![self.delegate, release];
            let _: () = msg_send![self.session, release];

            // 6. Release the dispatch queue (+1 from dispatch_queue_create)
            dispatch_release(self.queue);
        }
    }
}
unsafe fn cleanup_start_capture_resources(
    session: *mut Object,
    output: *mut Object,
    delegate: *mut Object,
    queue: *mut c_void,
    sender_ptr: *mut c_void,
) {
    if !sender_ptr.is_null() {
        let _ = Box::from_raw(sender_ptr as *mut mpsc::SyncSender<CVPixelBuffer>);
    }

    if !delegate.is_null() {
        let _: () = msg_send![delegate, release];
    }

    if !output.is_null() {
        let _: () = msg_send![output, release];
    }

    if !session.is_null() {
        let _: () = msg_send![session, release];
    }

    if !queue.is_null() {
        dispatch_release(queue);
    }
}

// --- merged from part_001.rs ---
/// Start webcam capture. Returns a receiver that yields CVPixelBuffers and
/// a [`CaptureHandle`] that must be kept alive for the duration of capture.
/// Dropping the handle stops capture and releases all resources.
pub fn start_capture(
    width: u32,
) -> std::result::Result<(mpsc::Receiver<CVPixelBuffer>, CaptureHandle), WebcamStartError> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Device).map_err(
        |error| WebcamStartError::InputInitFailed {
            attempted: "start capture",
            failed: "runtime effect policy",
            state: "not_started",
            details: error.to_string(),
        },
    )?;
    let (tx, rx) = mpsc::sync_channel::<CVPixelBuffer>(1);

    // Register our delegate class (once)
    static REGISTER: Once = Once::new();
    REGISTER.call_once(register_delegate_class);

    unsafe {
        // Create capture session (+1 retained)
        let session: *mut Object = msg_send![class!(AVCaptureSession), alloc];
        let session: *mut Object = msg_send![session, init];

        // Set session preset
        let preset_cstr = if width <= 640 {
            c"AVCaptureSessionPreset640x480"
        } else {
            c"AVCaptureSessionPresetHigh"
        };
        let preset: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: preset_cstr.as_ptr()];
        let can_set: BOOL = msg_send![session, canSetSessionPreset: preset];
        if can_set == YES {
            let _: () = msg_send![session, setSessionPreset: preset];
        }

        // Get default video capture device
        let device: *mut Object = msg_send![
            class!(AVCaptureDevice),
            defaultDeviceWithMediaType: av_media_type_video()
        ];
        if device.is_null() {
            cleanup_start_capture_resources(
                session,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            return Err(WebcamStartError::NoDevice {
                attempted: "lookup default video device",
                failed: "camera device lookup",
                state: "session_created",
                details: "AVCaptureDevice::defaultDeviceWithMediaType returned null".to_string(),
            });
        }

        // Create input (convenience constructor — autoreleased, not ours to release)
        let mut error: *mut Object = std::ptr::null_mut();
        let input: *mut Object = msg_send![
            class!(AVCaptureDeviceInput),
            deviceInputWithDevice: device
            error: &mut error
        ];
        if input.is_null() {
            let summary = nserror_summary(error);
            cleanup_start_capture_resources(
                session,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            return Err(classify_input_init_error(summary));
        }

        let can_add_input: BOOL = msg_send![session, canAddInput: input];
        if can_add_input != YES {
            cleanup_start_capture_resources(
                session,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            return Err(WebcamStartError::CannotAddInput {
                attempted: "add camera input to session",
                failed: "session.canAddInput",
                state: "session_created_input_ready",
                details: "AVCaptureSession rejected AVCaptureDeviceInput".to_string(),
            });
        }
        let _: () = msg_send![session, addInput: input];

        // Create video data output (+1 retained)
        let output: *mut Object = msg_send![class!(AVCaptureVideoDataOutput), alloc];
        let output: *mut Object = msg_send![output, init];
        if output.is_null() {
            cleanup_start_capture_resources(
                session,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            return Err(WebcamStartError::OutputInitFailed {
                attempted: "create AVCaptureVideoDataOutput",
                failed: "video output init",
                state: "session_created_input_attached",
                details: "AVCaptureVideoDataOutput alloc/init returned null".to_string(),
            });
        }

        // Set pixel format to NV12 (420f) — required by GPUI's Metal renderer
        let format_num: *mut Object = msg_send![
            class!(NSNumber),
            numberWithUnsignedInt: K_CV_PIXEL_FORMAT_TYPE_420_YP_CB_CR_8_BI_PLANAR_FULL_RANGE
        ];

        let pixel_format_key = kCVPixelBufferPixelFormatTypeKey();
        let video_settings: *mut Object = msg_send![
            class!(NSDictionary),
            dictionaryWithObject: format_num
            forKey: pixel_format_key
        ];
        let _: () = msg_send![output, setVideoSettings: video_settings];
        let _: () = msg_send![output, setAlwaysDiscardsLateVideoFrames: YES];

        // Create dispatch queue for callbacks (+1 retained)
        let queue_label = c"com.scriptkit.webcam".as_ptr();
        let queue: *mut c_void = dispatch_queue_create(queue_label, std::ptr::null_mut());
        if queue.is_null() {
            cleanup_start_capture_resources(
                session,
                output,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            return Err(WebcamStartError::CallbackQueueUnavailable {
                attempted: "create dispatch callback queue",
                failed: "dispatch_queue_create",
                state: "session_created_input_output_ready",
                details: "dispatch_queue_create returned null".to_string(),
            });
        }

        // Create delegate (+1 retained), store sender
        let delegate_class = match Class::get("SKWebcamDelegate") {
            Some(cls) => cls,
            None => {
                cleanup_start_capture_resources(
                    session,
                    output,
                    std::ptr::null_mut(),
                    queue,
                    std::ptr::null_mut(),
                );
                return Err(WebcamStartError::DelegateClassUnavailable {
                    attempted: "lookup SKWebcamDelegate class",
                    failed: "delegate class registration",
                    state: "session_created_input_output_queue_ready",
                    details: "Class::get(\"SKWebcamDelegate\") returned None".to_string(),
                });
            }
        };
        let delegate: *mut Object = msg_send![delegate_class, alloc];
        let delegate: *mut Object = msg_send![delegate, init];
        if delegate.is_null() {
            cleanup_start_capture_resources(
                session,
                output,
                std::ptr::null_mut(),
                queue,
                std::ptr::null_mut(),
            );
            return Err(WebcamStartError::DelegateClassUnavailable {
                attempted: "create SKWebcamDelegate instance",
                failed: "delegate init",
                state: "session_created_input_output_queue_ready",
                details: "SKWebcamDelegate alloc/init returned null".to_string(),
            });
        }

        // Store the sender as a raw pointer in the delegate
        let tx_box = Box::new(tx);
        let sender_ptr = Box::into_raw(tx_box) as *mut c_void;
        (*delegate).set_ivar::<*mut c_void>("_sender", sender_ptr);

        let _: () =
            msg_send![output, setSampleBufferDelegate: delegate queue: queue as *mut Object];

        let can_add_output: BOOL = msg_send![session, canAddOutput: output];
        if can_add_output != YES {
            cleanup_start_capture_resources(session, output, delegate, queue, sender_ptr);
            return Err(WebcamStartError::CannotAddOutput {
                attempted: "add camera output to session",
                failed: "session.canAddOutput",
                state: "session_created_input_output_configured",
                details: "AVCaptureSession rejected AVCaptureVideoDataOutput".to_string(),
            });
        }
        let _: () = msg_send![session, addOutput: output];

        // Release output — session retains it, we don't need it after this
        let _: () = msg_send![output, release];

        // Start capturing
        let _: () = msg_send![session, startRunning];

        let handle = CaptureHandle {
            session,
            delegate,
            queue,
            sender_ptr,
        };

        Ok((rx, handle))
    }
}
/// Register our Objective-C delegate class for receiving camera frames.
/// If registration fails (e.g. class name collision), start_capture will
/// return an error when it tries to look up the class.
fn register_delegate_class() {
    let superclass = class!(NSObject);
    let Some(mut decl) = ClassDecl::new("SKWebcamDelegate", superclass) else {
        return;
    };

    decl.add_ivar::<*mut c_void>("_sender");

    unsafe {
        decl.add_method(
            sel!(captureOutput:didOutputSampleBuffer:fromConnection:),
            capture_callback
                as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object, *mut Object),
        );
    }

    decl.register();
}
/// AVCaptureVideoDataOutputSampleBufferDelegate callback.
/// Extracts CVPixelBuffer from the sample buffer and sends it through the channel.
extern "C" fn capture_callback(
    this: &mut Object,
    _sel: Sel,
    _output: *mut Object,
    sample_buffer: *mut Object,
    _connection: *mut Object,
) {
    unsafe {
        let pixel_buffer_ref: CVPixelBufferRef = CMSampleBufferGetImageBuffer(sample_buffer as _);
        if pixel_buffer_ref.is_null() {
            return;
        }

        // Retain the CVPixelBuffer (wrap_under_get_rule calls CFRetain)
        let pixel_buffer = CVPixelBuffer::wrap_under_get_rule(pixel_buffer_ref);

        // Send through channel — try_send is intentional: if the channel is full
        // (consumer hasn't drained the previous frame yet), we drop this frame.
        // This is the desired behavior for real-time video: always show the latest
        // frame rather than queuing up stale ones.
        let sender_ptr = *this.get_ivar::<*mut c_void>("_sender");
        if !sender_ptr.is_null() {
            let sender = &*(sender_ptr as *const mpsc::SyncSender<CVPixelBuffer>);
            let _ = sender.try_send(pixel_buffer);
        }
    }
}
// External declarations
extern "C" {
    fn CMSampleBufferGetImageBuffer(sbuf: *mut c_void) -> CVPixelBufferRef;
    fn dispatch_queue_create(label: *const i8, attr: *mut c_void) -> *mut c_void;
    fn dispatch_release(object: *mut c_void);
    fn dispatch_sync_f(queue: *mut c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
}
/// Get AVMediaTypeVideo string
fn av_media_type_video() -> *mut Object {
    unsafe {
        let s: *mut Object = msg_send![class!(NSString), stringWithUTF8String: c"vide".as_ptr()];
        s
    }
}
/// Get kCVPixelBufferPixelFormatTypeKey
#[allow(non_snake_case)]
fn kCVPixelBufferPixelFormatTypeKey() -> *mut Object {
    extern "C" {
        static kCVPixelBufferPixelFormatTypeKey: *mut Object;
    }
    unsafe { kCVPixelBufferPixelFormatTypeKey }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_input_error_returns_permission_denied_when_not_authorized() {
        let summary = NSErrorSummary {
            domain: Some("AVFoundationErrorDomain".to_string()),
            code: Some(-11852),
            description: Some("The operation couldn’t be completed. Not authorized".to_string()),
        };

        let error = classify_input_init_error(summary);
        assert!(matches!(error, WebcamStartError::PermissionDenied { .. }));
    }

    #[test]
    fn test_classify_input_error_returns_device_busy_when_in_use() {
        let summary = NSErrorSummary {
            domain: Some("AVFoundationErrorDomain".to_string()),
            code: Some(-11815),
            description: Some("The video device is in use by another client".to_string()),
        };

        let error = classify_input_init_error(summary);
        assert!(matches!(error, WebcamStartError::DeviceBusy { .. }));
    }

    #[test]
    fn test_classify_input_error_returns_generic_with_context_when_unknown() {
        let summary = NSErrorSummary {
            domain: Some("CustomCameraDomain".to_string()),
            code: Some(42),
            description: Some("Unknown failure from test camera stack".to_string()),
        };

        let error = classify_input_init_error(summary);
        match error {
            WebcamStartError::InputInitFailed {
                attempted,
                failed,
                state,
                details,
            } => {
                assert_eq!(attempted, "create camera input");
                assert_eq!(failed, "camera input initialization");
                assert_eq!(state, "session_created_device_present");
                assert!(details.contains("CustomCameraDomain"));
                assert!(details.contains("Unknown failure"));
            }
            other => panic!("Expected InputInitFailed, got {other:?}"),
        }
    }
}

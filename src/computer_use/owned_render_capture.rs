use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Instant;

use anyhow::{ensure, Context as _, Result};
use gpui::{AnyWindowHandle, App};

use super::gpui_runtime_bridge::{capture_info_from_png, pixel_audit_from_platform};
use super::runtime_bridge::{
    ComputerUseCaptureNativeWindowError, ComputerUseCaptureRenderWindowRequest,
    ComputerUseCaptureRenderWindowSnapshot, ComputerUseCaptureRenderWindowStatus,
};
#[cfg(any(test, feature = "owned-ui-evaluation"))]
use crate::protocol::OwnedRuntimeIdentity;
use crate::protocol::{
    AutomationTargetIdentitySnapshot, AutomationWindowTarget, CompletedFrameIdentity,
    OWNED_EVALUATION_LIMITS,
};
use crate::runtime_policy::WindowHostPolicy;

/// Pixels extracted from the exact completed scene, on its native GPUI renderer.
/// Publish after leaving the window update so the facade can independently
/// revalidate that lifetime and the current production-owner identity.
pub struct OwnedCompletedRenderFrame {
    pub identity: CompletedFrameIdentity,
    /// Straight-alpha RGBA8 from the completed native readback, never flattened.
    pub image: image::RgbaImage,
    pub scale_factor: f32,
    pub phase_durations_ms: BTreeMap<String, f64>,
}

/// Native state of exactly the borrowed owned window and this process's app.
/// The lifetime address is internal capture identity, never wire evidence.
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnedNativeWindowObservation {
    native_window_id: i64,
    visible: bool,
    key: bool,
    miniaturized: bool,
    app_active: bool,
    #[serde(skip)]
    native_lifetime: usize,
}

impl OwnedNativeWindowObservation {
    fn require_nonpresenting(&self) -> Result<()> {
        ensure!(
            !self.visible && !self.key && !self.miniaturized,
            "owned_native_window_not_hidden"
        );
        ensure!(!self.app_active, "owned_native_app_active");
        Ok(())
    }
}

/// Observe only the live NSWindow borrowed by GPUI and the existing own NSApp.
/// No enumeration, frontmost-app discovery, activation, or native allocation.
pub(crate) fn observe_owned_native_window(
    window: &gpui::Window,
) -> Result<OwnedNativeWindowObservation> {
    ensure!(
        crate::runtime_policy::is_owned_evaluation() && window.is_owned_hidden(),
        "unqualified_render_window"
    );
    let raw = raw_window_handle::HasWindowHandle::window_handle(window)
        .map_err(|_| anyhow::anyhow!("native_render_lifetime_missing"))?;
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = raw.as_raw() else {
        anyhow::bail!("native_render_lifetime_missing");
    };
    // SAFETY: Called during an owned GPUI Window update on the app thread. The
    // NSView lends only its own NSWindow for these reads. NSApp is this process's
    // already initialized application, not the frontmost/another application's.
    unsafe {
        use cocoa::base::{BOOL, YES};
        use objc::{msg_send, sel, sel_impl};
        let view = appkit.ns_view.as_ptr() as *mut objc::runtime::Object;
        let native: *mut objc::runtime::Object = msg_send![view, window];
        ensure!(!native.is_null(), "native_render_lifetime_missing");
        let app = cocoa::appkit::NSApp();
        ensure!(!app.is_null(), "owned_native_app_missing");
        let native_window_id: i64 = msg_send![native, windowNumber];
        let visible: BOOL = msg_send![native, isVisible];
        let key: BOOL = msg_send![native, isKeyWindow];
        let miniaturized: BOOL = msg_send![native, isMiniaturized];
        let app_active: BOOL = msg_send![app, isActive];
        Ok(OwnedNativeWindowObservation {
            native_window_id,
            visible: visible == YES,
            key: key == YES,
            miniaturized: miniaturized == YES,
            app_active: app_active == YES,
            native_lifetime: native as usize,
        })
    }
}

type CurrentIdentity = Rc<dyn Fn(&mut App) -> Result<AutomationTargetIdentitySnapshot>>;

struct RetainedFrame {
    frame: OwnedCompletedRenderFrame,
    current_identity: CurrentIdentity,
    handle: AnyWindowHandle,
    native_lifetime: usize,
    audit: crate::platform::PixelAudit,
}

#[derive(Default)]
struct CompletedFrames {
    frames: BTreeMap<String, Rc<RetainedFrame>>,
    #[cfg(any(test, feature = "owned-ui-evaluation"))]
    runtime: Option<OwnedRuntimeIdentity>,
    #[cfg(any(test, feature = "owned-ui-evaluation"))]
    published_frames: u32,
    returned_images: u32,
}

thread_local! {
    // GPUI-thread-only ownership: callbacks and cached RGBA never cross threads.
    static COMPLETED_FRAMES: RefCell<CompletedFrames> = RefCell::new(CompletedFrames::default());
}

fn exact_instance(target: &AutomationWindowTarget) -> Result<(&str, u64)> {
    match target {
        AutomationWindowTarget::Instance { id, generation }
            if !id.is_empty() && *generation > 0 =>
        {
            Ok((id, *generation))
        }
        _ => anyhow::bail!("exact_window_instance_required"),
    }
}

fn validate_identity(frame: &CompletedFrameIdentity) -> Result<()> {
    let (id, generation) = exact_instance(&frame.requested_target)?;
    ensure!(
        frame.target.window_id == id && frame.target.window_generation == Some(generation),
        "frame_target_mismatch"
    );
    ensure!(
        !frame.target.app_view_variant.is_empty(),
        "frame_surface_missing"
    );
    ensure!(
        frame.target.presentation_revision.is_some() && frame.target.theme_revision.is_some(),
        "frame_revisions_missing"
    );
    ensure!(
        frame.target.frame_generation.is_some_and(|frame| frame > 0),
        "completed_frame_required"
    );
    ensure!(
        !frame.runtime.process_start_time.is_empty(),
        "process_start_time_missing"
    );
    for hash in [&frame.runtime.binary_sha256, &frame.runtime.manifest_sha256] {
        ensure!(
            hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "frame_artifact_identity_invalid"
        );
    }
    Ok(())
}

fn validate_process(frame: &CompletedFrameIdentity) -> Result<()> {
    validate_identity(frame)?;
    let policy = crate::runtime_policy::owned_evaluation().context("owned_evaluation_required")?;
    ensure!(
        frame.runtime.pid == std::process::id(),
        "frame_process_mismatch"
    );
    ensure!(
        frame.runtime.process_instance_id == policy.process_instance_id(),
        "frame_process_instance_mismatch"
    );
    ensure!(
        frame.runtime.session_generation == policy.session_generation(),
        "frame_session_mismatch"
    );
    Ok(())
}

fn validate_dimensions(width: u32, height: u32, scale_factor: f32) -> Result<()> {
    ensure!(
        width > 0
            && height > 0
            && u64::from(width) * u64::from(height)
                <= u64::from(OWNED_EVALUATION_LIMITS.max_image_pixels),
        "frame_pixel_budget_exhausted"
    );
    ensure!(
        scale_factor.is_finite() && scale_factor > 0.0,
        "frame_scale_invalid"
    );
    Ok(())
}

/// Validate the registered handle, its live native object, and the scene stamp.
/// No desktop enumeration, capture, showing, activation, or renderer invocation.
fn validate_lifetime(
    frame: &OwnedCompletedRenderFrame,
    cx: &mut App,
) -> Result<(AnyWindowHandle, usize)> {
    validate_process(&frame.identity)?;
    validate_dimensions(
        frame.image.width(),
        frame.image.height(),
        frame.scale_factor,
    )?;
    let (id, generation) = exact_instance(&frame.identity.requested_target)?;
    ensure!(
        crate::windows::runtime_window_host_policy(id, generation)?
            == WindowHostPolicy::OwnedHidden,
        "unqualified_render_window"
    );
    let info = crate::windows::automation_window_by_id(id).context("render_target_not_found")?;
    ensure!(
        info.generation == Some(generation) && info.pid == Some(std::process::id()),
        "render_target_lifetime_stale"
    );
    ensure!(
        !info.visible && !info.focused,
        "owned_window_visible_metadata"
    );
    if let Some(parent_id) = info.parent_window_id.as_deref() {
        let parent_generation = info
            .parent_window_generation
            .context("parent_generation_missing")?;
        ensure!(
            crate::windows::runtime_window_host_policy(parent_id, parent_generation)?
                == WindowHostPolicy::OwnedHidden,
            "parent_lifetime_stale"
        );
        let parent =
            crate::windows::get_runtime_window_handle_for_generation(parent_id, parent_generation)
                .context("parent_lifetime_stale")?;
        parent.update(cx, |_, window, _| {
            observe_owned_native_window(window)?.require_nonpresenting()
        })??;
    }
    let handle = crate::windows::get_runtime_window_handle_for_generation(id, generation)
        .context("render_target_not_found")?;
    let native_lifetime = handle.update(cx, |_, window, _| -> Result<usize> {
        ensure!(window.is_owned_hidden(), "unqualified_render_window");
        ensure!(
            Some(window.rendered_frame_generation()) == frame.identity.target.frame_generation,
            "capture_frame_identity_stale"
        );
        ensure!(
            window.scale_factor() == frame.scale_factor,
            "capture_scale_stale"
        );
        let size = window
            .viewport_size()
            .to_device_pixels(window.scale_factor());
        ensure!(
            size.width.0 > 0
                && size.height.0 > 0
                && size.width.0 as u32 == frame.image.width()
                && size.height.0 as u32 == frame.image.height(),
            "capture_geometry_stale"
        );
        let native = observe_owned_native_window(window)?;
        native.require_nonpresenting()?;
        if let Some(expected_number) = frame.identity.native_window_id {
            ensure!(
                expected_number > 0 && expected_number == native.native_window_id,
                "native_render_identity_stale"
            );
        }
        Ok(native.native_lifetime)
    })??;
    ensure!(
        frame.identity.target.theme_revision == Some(crate::theme::service::theme_revision()),
        "capture_theme_stale"
    );
    Ok((handle, native_lifetime))
}

/// Replaces only this exact lifetime's frame. A failed publication retires its
/// previous pixels; it can never leave an old success masquerading as the update.
#[cfg(any(test, feature = "owned-ui-evaluation"))]
pub fn publish_owned_render_frame(
    frame: OwnedCompletedRenderFrame,
    current_identity: CurrentIdentity,
    cx: &mut App,
) -> Result<()> {
    let (id, generation) = exact_instance(&frame.identity.requested_target)?;
    let key = id.to_owned();
    forget_owned_render_frame(id, generation);
    validate_process(&frame.identity)?;
    ensure!(
        frame.phase_durations_ms.len() <= 32
            && frame
                .phase_durations_ms
                .iter()
                .all(|(phase, duration)| !phase.is_empty()
                    && phase.len() <= 64
                    && duration.is_finite()
                    && *duration >= 0.0),
        "frame_timings_invalid"
    );
    let current = current_identity(cx)?;
    ensure!(
        current == frame.identity.target,
        "capture_frame_identity_stale"
    );
    let (handle, native_lifetime) = validate_lifetime(&frame, cx)?;
    let audit = crate::platform::audit_screenshot_pixels(&frame.image);
    COMPLETED_FRAMES.with(|store| {
        let mut store = store.borrow_mut();
        ensure!(
            store.frames.contains_key(&key)
                || store.frames.len() < OWNED_EVALUATION_LIMITS.max_windows as usize,
            "retained_frame_budget_exhausted"
        );
        ensure!(
            store.published_frames < OWNED_EVALUATION_LIMITS.max_frames,
            "completed_frame_budget_exhausted"
        );
        if let Some(runtime) = &store.runtime {
            ensure!(
                runtime == &frame.identity.runtime,
                "frame_runtime_identity_changed"
            );
        } else {
            store.runtime = Some(frame.identity.runtime.clone());
        }
        store.published_frames += 1;
        store.frames.insert(
            key,
            Rc::new(RetainedFrame {
                frame,
                current_identity,
                handle,
                native_lifetime,
                audit,
            }),
        );
        Ok(())
    })
}

/// Exact teardown deliberately does not reset the session's allocation counters.
pub fn forget_owned_render_frame(id: &str, generation: u64) -> bool {
    COMPLETED_FRAMES.with(|store| {
        let mut store = store.borrow_mut();
        if store.frames.get(id).is_some_and(|retained| {
            retained.frame.identity.target.window_generation == Some(generation)
        }) {
            store.frames.remove(id);
            true
        } else {
            false
        }
    })
}

fn failure(
    request: &ComputerUseCaptureRenderWindowRequest,
    status: ComputerUseCaptureRenderWindowStatus,
    code: &'static str,
    message: String,
) -> ComputerUseCaptureRenderWindowSnapshot {
    ComputerUseCaptureRenderWindowSnapshot {
        schema_version: 1,
        source: "gpuiRenderReadback",
        scope: "liveAutomationWindowRenderReadback",
        status,
        correlation_id: request.correlation_id.clone(),
        target: request.target.clone(),
        frame_identity: None,
        phase_durations_ms: BTreeMap::new(),
        capture: None,
        pixel_probes: Vec::new(),
        error: Some(ComputerUseCaptureNativeWindowError { code, message, reason: Some(code.to_owned()), pixel_audit: None }),
        warnings: vec!["No pixels were captured; do not count this as app-render visual proof.".to_owned()],
        limitation: "App-rendered GPUI pixels only; does not prove macOS WindowServer compositor/native blur output.",
    }
}

fn request_matches(
    request: &ComputerUseCaptureRenderWindowRequest,
    frame: &CompletedFrameIdentity,
) -> bool {
    request.target == frame.requested_target && request.expected.as_ref() == Some(&frame.target)
}

pub(crate) fn validate_current_frame_identity(
    current: &AutomationTargetIdentitySnapshot,
    expected: &AutomationTargetIdentitySnapshot,
) -> Result<()> {
    ensure!(current == expected, "capture_frame_identity_stale");
    Ok(())
}

/// Validate while the exact target window is already borrowed for deferred
/// input. Never re-enter its handle, pump work, redraw, or capture native pixels.
pub(crate) fn validate_owned_frame_for_input(
    expected: &CompletedFrameIdentity,
    current: &AutomationTargetIdentitySnapshot,
    window: &gpui::Window,
) -> Result<()> {
    validate_process(expected)?;
    validate_current_frame_identity(current, &expected.target)?;
    let (id, generation) = exact_instance(&expected.requested_target)?;
    let retained = COMPLETED_FRAMES
        .with(|store| store.borrow().frames.get(id).cloned())
        .context("completed_frame_required")?;
    ensure!(
        retained.frame.identity == *expected,
        "completed_frame_retired"
    );
    ensure!(
        window.window_handle() == retained.handle
            && crate::windows::get_runtime_window_handle_for_generation(id, generation)
                == Some(retained.handle),
        "render_target_lifetime_stale"
    );
    ensure!(
        crate::windows::runtime_window_host_policy(id, generation)?
            == WindowHostPolicy::OwnedHidden,
        "unqualified_render_window"
    );
    ensure!(
        Some(window.rendered_frame_generation()) == expected.target.frame_generation,
        "stale_frame_identity"
    );
    ensure!(
        window.scale_factor() == retained.frame.scale_factor,
        "capture_scale_stale"
    );
    let size = window
        .viewport_size()
        .to_device_pixels(window.scale_factor());
    ensure!(
        size.width.0 > 0
            && size.height.0 > 0
            && size.width.0 as u32 == retained.frame.image.width()
            && size.height.0 as u32 == retained.frame.image.height(),
        "capture_geometry_stale"
    );
    let native = observe_owned_native_window(window)?;
    native.require_nonpresenting()?;
    ensure!(
        native.native_lifetime == retained.native_lifetime,
        "native_render_lifetime_stale"
    );
    ensure!(
        expected.target.theme_revision == Some(crate::theme::service::theme_revision()),
        "capture_theme_stale"
    );
    Ok(())
}

fn validate_retained_frame(retained: &Rc<RetainedFrame>, cx: &mut App) -> Result<()> {
    ensure!(
        COMPLETED_FRAMES.with(|store| store
            .borrow()
            .frames
            .get(&retained.frame.identity.target.window_id)
            .is_some_and(|current| Rc::ptr_eq(current, retained))),
        "completed_frame_retired"
    );
    let current = (retained.current_identity)(cx)?;
    validate_current_frame_identity(&current, &retained.frame.identity.target)?;
    let (handle, native_lifetime) = validate_lifetime(&retained.frame, cx)?;
    ensure!(
        handle == retained.handle && native_lifetime == retained.native_lifetime,
        "native_render_lifetime_stale"
    );
    Ok(())
}

/// Read measurements belonging to an authoritative published scene. Queued
/// invalidation of its next frame is allowed, but changing or retiring this
/// exact owner/native scene at either boundary is not. This never pumps work.
#[cfg(any(test, feature = "owned-ui-evaluation"))]
pub(crate) fn with_owned_completed_frame<T>(
    expected: &CompletedFrameIdentity,
    cx: &mut App,
    read: impl FnOnce(&CompletedFrameIdentity, &mut App) -> Result<T>,
) -> Result<T> {
    validate_identity(expected)?;
    let retained = COMPLETED_FRAMES
        .with(|store| {
            store
                .borrow()
                .frames
                .get(&expected.target.window_id)
                .cloned()
        })
        .context("completed_frame_required")?;
    ensure!(
        retained.frame.identity == *expected,
        "capture_frame_identity_stale"
    );
    validate_retained_frame(&retained, cx)?;
    let result = read(&retained.frame.identity, cx);
    validate_retained_frame(&retained, cx)?;
    result
}

fn output_image(frame: &OwnedCompletedRenderFrame, hi_dpi: bool) -> Cow<'_, image::RgbaImage> {
    if !hi_dpi && frame.scale_factor > 1.0 {
        let width = ((frame.image.width() as f32 / frame.scale_factor).round() as u32).max(1);
        let height = ((frame.image.height() as f32 / frame.scale_factor).round() as u32).max(1);
        // image::resize requires premultiplied color. Sample a view instead of
        // allocating or mutating another full-size copy of the retained frame.
        // BGRA order lets the existing GPUI converter restore straight RGBA
        // once, in place, after filtering.
        struct PremultipliedBgraView<'a>(&'a image::RgbaImage);
        impl image::GenericImageView for PremultipliedBgraView<'_> {
            type Pixel = image::Rgba<u8>;

            fn dimensions(&self) -> (u32, u32) {
                self.0.dimensions()
            }

            fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
                let [r, g, b, a] = self.0.get_pixel(x, y).0;
                let premultiply = |c: u8| ((u16::from(c) * u16::from(a) + 127) / 255) as u8;
                image::Rgba([premultiply(b), premultiply(g), premultiply(r), a])
            }
        }

        let mut resized = image::imageops::resize(
            &PremultipliedBgraView(&frame.image),
            width,
            height,
            image::imageops::FilterType::Lanczos3,
        );
        for pixel in resized.pixels_mut() {
            if pixel.0[3] == 0 {
                pixel.0.fill(0);
            } else {
                gpui::swap_rgba_pa_to_bgra(&mut pixel.0);
            }
        }
        Cow::Owned(resized)
    } else {
        Cow::Borrowed(&frame.image)
    }
}

const MAX_RENDER_PIXEL_PROBES: usize = 64;

pub(crate) fn sample_retained_pixel(
    frame: &OwnedCompletedRenderFrame,
    probe: &crate::protocol::PixelProbe,
) -> Result<crate::protocol::PixelProbeResult> {
    ensure!(
        probe.x < frame.image.width() && probe.y < frame.image.height(),
        "pixel_probe_out_of_bounds"
    );
    let [r, g, b, a] = frame.image.get_pixel(probe.x, probe.y).0;
    Ok(crate::protocol::PixelProbeResult {
        x: probe.x,
        y: probe.y,
        r,
        g,
        b,
        a,
    })
}

fn sample_retained_pixels(
    frame: &OwnedCompletedRenderFrame,
    probes: &[crate::protocol::PixelProbe],
) -> Result<Vec<crate::protocol::PixelProbeResult>> {
    ensure!(
        probes.len() <= MAX_RENDER_PIXEL_PROBES,
        "pixel_probe_budget_exhausted"
    );
    probes
        .iter()
        .map(|probe| sample_retained_pixel(frame, probe))
        .collect()
}

pub(super) fn capture(
    request: &ComputerUseCaptureRenderWindowRequest,
    cx: &mut App,
) -> ComputerUseCaptureRenderWindowSnapshot {
    use ComputerUseCaptureRenderWindowStatus as Status;
    let started = Instant::now();
    let (id, generation) = match exact_instance(&request.target) {
        Ok(instance) => instance,
        Err(error) => {
            return failure(
                request,
                Status::CaptureFailed,
                "exact_window_instance_required",
                error.to_string(),
            )
        }
    };
    if request.expected.is_none() {
        return failure(
            request,
            Status::CaptureFailed,
            "expected_identity_required",
            "Qualified readback requires the exact completed-frame target identity".into(),
        );
    }
    let retained = COMPLETED_FRAMES.with(|store| {
        store
            .borrow()
            .frames
            .get(id)
            .filter(|retained| retained.frame.identity.target.window_generation == Some(generation))
            .cloned()
    });
    let Some(retained) = retained else {
        return failure(
            request,
            Status::TargetNotFound,
            "completed_frame_required",
            "No completed frame is retained for this exact lifetime".into(),
        );
    };
    if !request_matches(request, &retained.frame.identity) {
        return failure(
            request,
            Status::CaptureFailed,
            "capture_frame_identity_stale",
            "Expected identity does not exactly match the retained scene".into(),
        );
    }
    let validity = validate_retained_frame(&retained, cx);
    if let Err(error) = validity {
        forget_owned_render_frame(id, generation);
        return failure(
            request,
            Status::CaptureFailed,
            "capture_frame_identity_stale",
            error.to_string(),
        );
    }
    if request.include_image
        && COMPLETED_FRAMES.with(|store| {
            store.borrow().returned_images >= OWNED_EVALUATION_LIMITS.max_retained_images
        })
    {
        return failure(
            request,
            Status::CaptureFailed,
            "retained_image_budget_exhausted",
            "Session image response budget exhausted".into(),
        );
    }
    let pixel_probes = match sample_retained_pixels(&retained.frame, &request.probes) {
        Ok(probes) => probes,
        Err(error) => {
            return failure(
                request,
                Status::CaptureFailed,
                "invalid_pixel_probes",
                error.to_string(),
            )
        }
    };
    let image = output_image(&retained.frame, request.hi_dpi);
    let audit = match &image {
        Cow::Borrowed(_) => retained.audit.clone(),
        Cow::Owned(image) => crate::platform::audit_screenshot_pixels(image),
    };
    if retained.audit.is_blank_like() || audit.is_blank_like() {
        let mut snapshot = failure(
            request,
            Status::BlankImageRejected,
            "blank_image_rejected",
            "Completed GPUI pixels failed the existing blank-image audit".into(),
        );
        if let Some(error) = snapshot.error.as_mut() {
            error.pixel_audit = Some(pixel_audit_from_platform(&audit));
        }
        return snapshot;
    }
    let encode_started = Instant::now();
    let encoded = match crate::platform::encode_screenshot_png(
        &image,
        audit,
        OWNED_EVALUATION_LIMITS.max_png_bytes as usize,
    ) {
        Ok(encoded) => encoded,
        Err(error) => {
            return failure(
                request,
                Status::CaptureFailed,
                "png_encode_failed",
                error.to_string(),
            )
        }
    };
    let capture = capture_info_from_png(&encoded, request.hi_dpi, request.include_image);
    if request.include_image {
        COMPLETED_FRAMES.with(|store| store.borrow_mut().returned_images += 1);
    }
    let mut phases = retained.frame.phase_durations_ms.clone();
    phases.insert(
        "pngEncodeAndMetadata".into(),
        encode_started.elapsed().as_secs_f64() * 1000.0,
    );
    phases.insert("capture".into(), started.elapsed().as_secs_f64() * 1000.0);
    ComputerUseCaptureRenderWindowSnapshot {
        schema_version: 1,
        source: "gpuiRenderReadback",
        scope: "liveAutomationWindowRenderReadback",
        status: Status::Captured,
        correlation_id: request.correlation_id.clone(),
        target: request.target.clone(),
        frame_identity: Some(retained.frame.identity.clone()),
        phase_durations_ms: phases,
        capture: Some(capture),
        pixel_probes,
        error: None,
        warnings: Vec::new(),
        limitation: "App-rendered GPUI pixels only; does not prove macOS WindowServer compositor/native blur output.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> OwnedCompletedRenderFrame {
        OwnedCompletedRenderFrame {
            identity: CompletedFrameIdentity {
                runtime: OwnedRuntimeIdentity {
                    pid: std::process::id(),
                    process_start_time: "owned-start".into(),
                    process_instance_id: "owned-process".into(),
                    session_generation: "owned-session".into(),
                    binary_sha256: "a".repeat(64),
                    manifest_sha256: "b".repeat(64),
                },
                requested_target: AutomationWindowTarget::Instance {
                    id: "main".into(),
                    generation: 7,
                },
                target: AutomationTargetIdentitySnapshot {
                    window_id: "main".into(),
                    window_generation: Some(7),
                    app_view_variant: "ScriptList".into(),
                    target_generation: 2,
                    surface_generation: 3,
                    data_generation: 4,
                    presentation_revision: Some(5),
                    theme_revision: Some(6),
                    frame_generation: Some(8),
                },
                native_window_id: None,
            },
            image: image::RgbaImage::from_fn(12, 8, |x, y| {
                image::Rgba([(x * 20) as u8, (y * 30) as u8, 128, 255])
            }),
            scale_factor: 2.0,
            phase_durations_ms: BTreeMap::from([("completedFrame".into(), 2.5)]),
        }
    }

    #[test]
    fn native_qualification_refuses_each_presenting_or_active_state() {
        for flags in 0_u8..16 {
            let observation = OwnedNativeWindowObservation {
                native_window_id: 7,
                visible: flags & 1 != 0,
                key: flags & 2 != 0,
                miniaturized: flags & 4 != 0,
                app_active: flags & 8 != 0,
                native_lifetime: 0x1234,
            };
            assert_eq!(observation.require_nonpresenting().is_ok(), flags == 0);
        }
    }

    #[test]
    fn native_observation_never_serializes_the_borrowed_pointer() {
        let observation = OwnedNativeWindowObservation {
            native_window_id: 7,
            visible: false,
            key: false,
            miniaturized: false,
            app_active: false,
            native_lifetime: 0x1234,
        };
        assert_eq!(
            serde_json::to_value(observation).unwrap(),
            serde_json::json!({
                "nativeWindowId":7,"visible":false,"key":false,"miniaturized":false,"appActive":false
            })
        );
    }

    #[test]
    fn capture_requires_every_expected_identity_field_to_match() {
        let frame = frame();
        let mut request = ComputerUseCaptureRenderWindowRequest {
            target: frame.identity.requested_target.clone(),
            expected: Some(frame.identity.target.clone()),
            hi_dpi: false,
            include_image: false,
            probes: Vec::new(),
            correlation_id: "capture-test".into(),
        };
        assert!(request_matches(&request, &frame.identity));
        let mutations: &[fn(&mut AutomationTargetIdentitySnapshot)] = &[
            |identity| identity.window_id.push_str("-other"),
            |identity| identity.window_generation = Some(9),
            |identity| identity.app_view_variant = "Notes".into(),
            |identity| identity.target_generation += 1,
            |identity| identity.surface_generation += 1,
            |identity| identity.data_generation += 1,
            |identity| identity.presentation_revision = Some(9),
            |identity| identity.theme_revision = Some(9),
            |identity| identity.frame_generation = Some(9),
            |identity| identity.frame_generation = None,
        ];
        for mutate in mutations {
            request.expected = Some(frame.identity.target.clone());
            mutate(request.expected.as_mut().unwrap());
            assert!(!request_matches(&request, &frame.identity));
        }
        request.expected = None;
        assert!(!request_matches(&request, &frame.identity));
        request.expected = Some(frame.identity.target.clone());
        request.target = AutomationWindowTarget::Id { id: "main".into() };
        assert!(!request_matches(&request, &frame.identity));
    }

    #[test]
    fn qualified_identity_rejects_missing_frame_and_lifetime() {
        let mut identity = frame().identity;
        assert!(validate_identity(&identity).is_ok());
        identity.target.frame_generation = Some(0);
        assert!(validate_identity(&identity).is_err());
        identity.target.frame_generation = Some(8);
        identity.target.window_generation = None;
        assert!(validate_identity(&identity).is_err());
        identity.target.window_generation = Some(7);
        identity.requested_target = AutomationWindowTarget::Focused;
        assert!(validate_identity(&identity).is_err());
    }

    #[test]
    fn coordinate_request_round_trip_preserves_exact_completed_frame_authority() {
        let frame = frame().identity;
        let request = serde_json::json!({"type":"simulateGpuiEvent","requestId":"owned-pointer",
            "target":frame.requested_target,"expected":frame.target,"expectedFrame":frame,
            "event":{"type":"mouseDown","x":2.0,"y":3.0,"button":"left"}});
        let message: crate::protocol::Message = serde_json::from_value(request.clone()).unwrap();
        let round_trip = serde_json::to_value(message).unwrap();
        assert_eq!(round_trip["expectedFrame"], request["expectedFrame"]);
        assert_eq!(round_trip["expected"], request["expected"]);
        let old_request = crate::protocol::Message::simulate_gpui_event(
            "old-key".into(),
            crate::protocol::SimulatedGpuiEvent::KeyDown {
                key: "a".into(),
                modifiers: vec![],
                text: None,
            },
            None,
        );
        assert!(serde_json::to_value(old_request)
            .unwrap()
            .get("expectedFrame")
            .is_none());
    }

    #[test]
    fn batch_round_trip_preserves_original_expected_identity() {
        let expected = frame().identity.target;
        let request = serde_json::json!({"type":"batch","requestId":"owned-batch","expected":expected,
            "commands":[{"type":"setInput","text":"first"},{"type":"setInput","text":"second"}]});
        let message: crate::protocol::Message = serde_json::from_value(request.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(message).unwrap()["expected"],
            request["expected"]
        );
    }

    #[gpui::test]
    fn completed_frame_measurements_require_published_authority(cx: &mut gpui::TestAppContext) {
        let mut expected = frame().identity;
        expected.target.window_id = "unpublished-completed-frame-measurement".into();
        expected.requested_target = AutomationWindowTarget::Instance {
            id: expected.target.window_id.clone(),
            generation: expected.target.window_generation.unwrap(),
        };
        let mut read_invoked = false;
        let result = cx.update(|cx| {
            with_owned_completed_frame(&expected, cx, |_, _| {
                read_invoked = true;
                Ok(())
            })
        });
        assert_eq!(result.unwrap_err().to_string(), "completed_frame_required");
        assert!(
            !read_invoked,
            "caller-supplied frame must never authorize a layout read"
        );
    }

    #[test]
    fn readback_dimensions_are_bounded_before_processing() {
        assert!(validate_dimensions(2048, 2048, 2.0).is_ok());
        for (width, height, scale) in [
            (2049, 2048, 2.0),
            (0, 8, 2.0),
            (8, 0, 2.0),
            (8, 8, 0.0),
            (8, 8, f32::NAN),
            (8, 8, f32::INFINITY),
        ] {
            assert!(validate_dimensions(width, height, scale).is_err());
        }
    }

    #[test]
    fn capture_uses_actual_scale_and_preserves_hidpi_pixels() {
        let mut frame = frame();
        let hidpi = output_image(&frame, true);
        assert!(matches!(hidpi, Cow::Borrowed(_)));
        assert_eq!(hidpi.dimensions(), (12, 8));
        assert_eq!(*hidpi, frame.image);
        assert_eq!(output_image(&frame, false).dimensions(), (6, 4));
        frame.scale_factor = 1.5;
        assert_eq!(output_image(&frame, false).dimensions(), (8, 5));
        frame.scale_factor = 1.0;
        assert!(matches!(output_image(&frame, false), Cow::Borrowed(_)));
    }

    #[test]
    fn probes_sample_native_straight_rgba_without_rescaling() {
        let mut frame = frame();
        frame.image.put_pixel(11, 7, image::Rgba([201, 99, 17, 64]));
        let samples = sample_retained_pixels(
            &frame,
            &[
                crate::protocol::PixelProbe { x: 11, y: 7 },
                crate::protocol::PixelProbe { x: 0, y: 0 },
            ],
        )
        .unwrap();
        assert_eq!(
            samples,
            vec![
                crate::protocol::PixelProbeResult {
                    x: 11,
                    y: 7,
                    r: 201,
                    g: 99,
                    b: 17,
                    a: 64
                },
                crate::protocol::PixelProbeResult {
                    x: 0,
                    y: 0,
                    r: 0,
                    g: 0,
                    b: 128,
                    a: 255
                },
            ]
        );
        assert_eq!(frame.image.dimensions(), (12, 8));
        assert_eq!(frame.image.get_pixel(11, 7).0, [201, 99, 17, 64]);
    }

    #[test]
    fn probes_refuse_out_of_bounds_and_over_budget_without_partial_results() {
        let frame = frame();
        let origin = crate::protocol::PixelProbe { x: 0, y: 0 };
        assert!(
            sample_retained_pixels(&frame, &vec![origin.clone(); MAX_RENDER_PIXEL_PROBES]).is_ok()
        );
        assert_eq!(
            sample_retained_pixels(&frame, &vec![origin.clone(); MAX_RENDER_PIXEL_PROBES + 1])
                .unwrap_err()
                .to_string(),
            "pixel_probe_budget_exhausted"
        );
        for probe in [
            crate::protocol::PixelProbe { x: 12, y: 0 },
            crate::protocol::PixelProbe { x: 0, y: 8 },
        ] {
            assert_eq!(
                sample_retained_pixels(&frame, &[origin.clone(), probe])
                    .unwrap_err()
                    .to_string(),
                "pixel_probe_out_of_bounds"
            );
        }
    }

    #[test]
    fn downscale_preserves_straight_color_at_transparent_edges() {
        let mut frame = frame();
        frame.image = image::RgbaImage::from_fn(2, 2, |x, _| {
            if x == 0 {
                image::Rgba([255, 0, 0, 255])
            } else {
                // Hidden blue must not leak into the visible red edge.
                image::Rgba([0, 0, 255, 0])
            }
        });
        let image = output_image(&frame, false);
        assert_eq!(image.dimensions(), (1, 1));
        let [r, g, b, a] = image.get_pixel(0, 0).0;
        // The shared GPUI unpremultiply truncates floating-point channels.
        assert!((254..=255).contains(&r));
        assert_eq!((g, b), (0, 0));
        assert!((127..=128).contains(&a));
        assert_eq!(frame.image.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(frame.image.get_pixel(1, 0).0, [0, 0, 255, 0]);

        frame.image = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 255, 0]));
        assert_eq!(output_image(&frame, false).get_pixel(0, 0).0, [0, 0, 0, 0]);
    }

    #[test]
    fn png_preserves_straight_alpha_without_multiplying_color_again() {
        let image =
            image::RgbaImage::from_raw(3, 1, vec![255, 0, 0, 64, 0, 255, 0, 128, 0, 0, 255, 255])
                .unwrap();
        let audit = crate::platform::audit_screenshot_pixels(&image);
        let encoded = crate::platform::encode_screenshot_png(&image, audit, 4096).unwrap();
        let decoded = image::load_from_memory(&encoded.png_data)
            .unwrap()
            .to_rgba8();
        assert_eq!(decoded, image);
    }

    #[test]
    fn bounded_png_and_metadata_describe_the_actual_pixels() {
        use base64::Engine as _;
        let frame = frame();
        let image = output_image(&frame, false);
        let audit = crate::platform::audit_screenshot_pixels(&image);
        assert!(!audit.is_blank_like());
        let encoded = crate::platform::encode_screenshot_png(&image, audit.clone(), 4096).unwrap();
        for limit in [0, encoded.png_data.len() - 12, encoded.png_data.len() - 1] {
            assert!(
                crate::platform::encode_screenshot_png(&image, audit.clone(), limit).is_err(),
                "PNG output must fail closed at byte limit {limit}"
            );
        }
        let exact =
            crate::platform::encode_screenshot_png(&image, audit, encoded.png_data.len()).unwrap();
        assert_eq!(exact.png_data, encoded.png_data);
        let metadata = capture_info_from_png(&encoded, false, false);
        let with_image = capture_info_from_png(&encoded, false, true);
        assert!(metadata.png_base64.is_none());
        assert_eq!(metadata.sha256, with_image.sha256);
        assert_eq!(metadata.byte_length, encoded.png_data.len());
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(with_image.png_base64.unwrap())
            .unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(decoded, *image);
        assert_eq!((metadata.width, metadata.height), decoded.dimensions());
    }
}

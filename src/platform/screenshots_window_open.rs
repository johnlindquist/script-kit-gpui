// ============================================================================
// Screenshot Capture
// ============================================================================

use image::codecs::png::PngEncoder;
use image::ImageEncoder;
use xcap::Window;

const DARK_EMPTY_MAX_UNIQUE_BUCKETS: usize = 2;
const DARK_EMPTY_MAX_MEAN_LUMA: f64 = 5.0;
const DARK_EMPTY_MAX_NON_BLACK_RATIO: f64 = 0.001;
const DARK_EMPTY_MAX_LUMA: f64 = 16.0;
const NON_BLACK_CHANNEL_THRESHOLD: u8 = 8;
const PIXEL_AUDIT_LUMA_RED_WEIGHT: f64 = 0.2126;
const PIXEL_AUDIT_LUMA_GREEN_WEIGHT: f64 = 0.7152;
const PIXEL_AUDIT_LUMA_BLUE_WEIGHT: f64 = 0.0722;
const PIXEL_AUDIT_COLOR_BUCKET_SIZE: u8 = 32;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PixelAudit {
    pub sampled: u64,
    pub non_black: u64,
    pub non_transparent: u64,
    pub unique_bucket_count: usize,
    pub mean_luma: f64,
    pub max_luma: f64,
    pub non_black_ratio: f64,
}

impl PixelAudit {
    pub(crate) fn is_blank_like(&self) -> bool {
        let solid_like = self.unique_bucket_count <= 1;
        let sparse_dark_capture_like = self.mean_luma < DARK_EMPTY_MAX_MEAN_LUMA
            && self.non_black_ratio < DARK_EMPTY_MAX_NON_BLACK_RATIO;
        let dark_empty_like = (self.unique_bucket_count <= DARK_EMPTY_MAX_UNIQUE_BUCKETS
            || sparse_dark_capture_like)
            && self.mean_luma < DARK_EMPTY_MAX_MEAN_LUMA
            && self.non_black_ratio < DARK_EMPTY_MAX_NON_BLACK_RATIO
            && (self.max_luma < DARK_EMPTY_MAX_LUMA || sparse_dark_capture_like);

        self.sampled == 0
            || self.non_transparent == 0
            || solid_like
            || dark_empty_like
    }
}

pub(crate) fn audit_screenshot_pixels(image: &image::RgbaImage) -> PixelAudit {
    let mut sampled = 0_u64;
    let mut non_black = 0_u64;
    let mut non_transparent = 0_u64;
    let mut luma_sum = 0_f64;
    let mut max_luma = 0_f64;
    let mut buckets = std::collections::HashSet::new();

    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        sampled += 1;

        if a > 0 {
            non_transparent += 1;
        }

        if a > 0 && (r > NON_BLACK_CHANNEL_THRESHOLD || g > NON_BLACK_CHANNEL_THRESHOLD || b > NON_BLACK_CHANNEL_THRESHOLD) {
            non_black += 1;
        }

        let luma = PIXEL_AUDIT_LUMA_RED_WEIGHT * f64::from(r) + PIXEL_AUDIT_LUMA_GREEN_WEIGHT * f64::from(g) + PIXEL_AUDIT_LUMA_BLUE_WEIGHT * f64::from(b);
        luma_sum += luma;
        max_luma = max_luma.max(luma);

        let bucket = (
            r / PIXEL_AUDIT_COLOR_BUCKET_SIZE,
            g / PIXEL_AUDIT_COLOR_BUCKET_SIZE,
            b / PIXEL_AUDIT_COLOR_BUCKET_SIZE,
            if a == 0 { 0 } else { 1 },
        );
        buckets.insert(bucket);
    }

    PixelAudit {
        sampled,
        non_black,
        non_transparent,
        unique_bucket_count: buckets.len(),
        mean_luma: if sampled == 0 {
            0.0
        } else {
            luma_sum / sampled as f64
        },
        max_luma,
        non_black_ratio: if sampled == 0 {
            0.0
        } else {
            non_black as f64 / sampled as f64
        },
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NativeWindowScreenshotCapture {
    pub png_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pixel_audit: PixelAudit,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NativeWindowCaptureError {
    PermissionDenied {
        message: String,
    },
    BlankImageRejected {
        audit: PixelAudit,
        message: String,
    },
    NativeWindowNotFound {
        native_window_id: u32,
    },
    OwnershipMismatch {
        native_window_id: u32,
        expected_pid: i32,
        actual_pid: i32,
    },
    AmbiguousNativeWindowId {
        native_window_id: u32,
        count: usize,
    },
    CaptureFailed {
        message: String,
    },
}

impl std::fmt::Display for NativeWindowCaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PermissionDenied { message }
            | Self::BlankImageRejected { message, .. }
            | Self::CaptureFailed { message } => write!(f, "{message}"),
            Self::NativeWindowNotFound { native_window_id } => {
                write!(
                    f,
                    "Native window {native_window_id} was not found in xcap inventory"
                )
            }
            Self::OwnershipMismatch {
                native_window_id,
                expected_pid,
                actual_pid,
            } => write!(
                f,
                "Native window {native_window_id} belongs to pid {actual_pid}, not expected pid {expected_pid}"
            ),
            Self::AmbiguousNativeWindowId {
                native_window_id,
                count,
            } => write!(
                f,
                "Native window {native_window_id} matched {count} xcap windows; refusing to guess"
            ),
        }
    }
}

impl std::error::Error for NativeWindowCaptureError {}

fn reject_blank_screenshot_if_needed(
    audit: &PixelAudit,
    correlation_id: &str,
) -> Result<(), NativeWindowCaptureError> {
    tracing::info!(
        target: "script_kit::automation",
        correlation_id = %correlation_id,
        sampled = audit.sampled,
        non_black = audit.non_black,
        non_transparent = audit.non_transparent,
        unique_bucket_count = audit.unique_bucket_count,
        mean_luma = audit.mean_luma,
        blank_like = audit.is_blank_like(),
        "automation.capture_screenshot.pixel_audit"
    );

    if audit.is_blank_like() {
        tracing::error!(
            target: "script_kit::automation",
            correlation_id = %correlation_id,
            sampled = audit.sampled,
            non_black = audit.non_black,
            non_transparent = audit.non_transparent,
            unique_bucket_count = audit.unique_bucket_count,
            mean_luma = audit.mean_luma,
            "automation.capture_screenshot.blank_image_rejected"
        );
        return Err(NativeWindowCaptureError::BlankImageRejected {
            audit: audit.clone(),
            message: "Screenshot capture returned a blank/black image. Check macOS Screen Recording permission for this binary and retry with a reviewable capture."
                .to_string(),
        });
    }

    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn screen_capture_access_preflight() -> Option<bool> {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }

    Some(unsafe { CGPreflightScreenCaptureAccess() })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn screen_capture_access_preflight() -> Option<bool> {
    None
}

#[cfg(target_os = "macos")]
pub(crate) fn event_synthesizing_access_preflight() -> Option<bool> {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightPostEventAccess() -> bool;
    }

    Some(unsafe { CGPreflightPostEventAccess() })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn event_synthesizing_access_preflight() -> Option<bool> {
    None
}

fn capture_and_encode_png(
    window: &xcap::Window,
    hi_dpi: bool,
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    let captured = capture_and_encode_png_with_audit(window, hi_dpi, "legacy-targeted-screenshot")
        .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> {
            error.to_string().into()
        })?;

    Ok((captured.png_data, captured.width, captured.height))
}

fn capture_and_encode_png_with_audit(
    window: &xcap::Window,
    hi_dpi: bool,
    correlation_id: &str,
) -> Result<NativeWindowScreenshotCapture, NativeWindowCaptureError> {
    const DOWNSCALE_DIVISOR: u32 = 2;

    if matches!(screen_capture_access_preflight(), Some(false)) {
        tracing::error!(
            target: "script_kit::automation",
            correlation_id = %correlation_id,
            "automation.capture_screenshot.permission_failed"
        );
        return Err(NativeWindowCaptureError::PermissionDenied {
            message: "Screen capture permission is not granted. Enable macOS Screen Recording permission for this binary before collecting screenshot proof."
                .to_string(),
        });
    }

    let image =
        window
            .capture_image()
            .map_err(|error| NativeWindowCaptureError::CaptureFailed {
                message: error.to_string(),
            })?;
    let original_width = image.width();
    let original_height = image.height();

    let (final_image, width, height) = if hi_dpi {
        (image, original_width, original_height)
    } else {
        let new_width = (original_width / DOWNSCALE_DIVISOR).max(1);
        let new_height = (original_height / DOWNSCALE_DIVISOR).max(1);
        let resized = image::imageops::resize(
            &image,
            new_width,
            new_height,
            image::imageops::FilterType::Lanczos3,
        );
        tracing::debug!(
            target: "script_kit::automation",
            correlation_id = %correlation_id,
            original_width = original_width,
            original_height = original_height,
            new_width = new_width,
            new_height = new_height,
            downscale_divisor = DOWNSCALE_DIVISOR,
            "automation.capture_screenshot.scaled_to_1x"
        );
        (resized, new_width, new_height)
    };

    let audit = audit_screenshot_pixels(&final_image);
    reject_blank_screenshot_if_needed(&audit, correlation_id)?;

    let mut png_data = Vec::new();
    let encoder = PngEncoder::new(&mut png_data);
    encoder
        .write_image(&final_image, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|error| NativeWindowCaptureError::CaptureFailed {
            message: error.to_string(),
        })?;

    Ok(NativeWindowScreenshotCapture {
        png_data,
        width,
        height,
        pixel_audit: audit,
    })
}

pub(crate) fn capture_native_window_id_screenshot(
    native_window_id: u32,
    expected_owner_pid: i32,
    hi_dpi: bool,
    correlation_id: &str,
) -> Result<NativeWindowScreenshotCapture, NativeWindowCaptureError> {
    let windows = Window::all().map_err(|error| NativeWindowCaptureError::CaptureFailed {
        message: error.to_string(),
    })?;
    let matches = windows
        .into_iter()
        .filter(|window| window.id().ok() == Some(native_window_id))
        .collect::<Vec<_>>();

    match matches.len() {
        0 => Err(NativeWindowCaptureError::NativeWindowNotFound { native_window_id }),
        1 => {
            let final_owner_pid = core_graphics_owner_pid_for_native_window(native_window_id)?;
            if final_owner_pid != expected_owner_pid {
                tracing::warn!(
                    target: "script_kit::automation",
                    correlation_id = %correlation_id,
                    native_window_id = native_window_id,
                    expected_pid = expected_owner_pid,
                    actual_pid = final_owner_pid,
                    "automation.native_window_capture.final_owner_mismatch"
                );
                return Err(NativeWindowCaptureError::OwnershipMismatch {
                    native_window_id,
                    expected_pid: expected_owner_pid,
                    actual_pid: final_owner_pid,
                });
            }
            tracing::info!(
                target: "script_kit::automation",
                correlation_id = %correlation_id,
                native_window_id = native_window_id,
                owner_pid = final_owner_pid,
                "automation.native_window_capture.xcap_window_selected"
            );
            capture_and_encode_png_with_audit(&matches[0], hi_dpi, correlation_id)
        }
        count => Err(NativeWindowCaptureError::AmbiguousNativeWindowId {
            native_window_id,
            count,
        }),
    }
}

#[cfg(target_os = "macos")]
fn core_graphics_owner_pid_for_native_window(
    native_window_id: u32,
) -> Result<i32, NativeWindowCaptureError> {
    use core_foundation::array::CFArray;
    use core_foundation::base::{CFTypeRef, TCFType};
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use std::ffi::c_void;

    const K_CG_NULL_WINDOW_ID: u32 = 0;
    const K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY: u32 = 1;

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
        CGWindowListCopyWindowInfo(K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY, K_CG_NULL_WINDOW_ID)
    };
    if window_info_list.is_null() {
        return Err(NativeWindowCaptureError::CaptureFailed {
            message: "CGWindowListCopyWindowInfo returned null during final owner check"
                .to_string(),
        });
    }

    let info_array: CFArray = unsafe { CFArray::wrap_under_create_rule(window_info_list) };
    let k_owner_pid = CFString::new("kCGWindowOwnerPID");
    let k_window_number = CFString::new("kCGWindowNumber");
    let mut owner_pids = Vec::new();

    for index in 0..info_array.len() {
        let Some(item_ref) = info_array.get(index) else {
            continue;
        };
        let dict_ref = *item_ref as CFDictionaryRef;
        if dict_ref.is_null() {
            continue;
        }

        let mut value: *const c_void = std::ptr::null();
        let found = unsafe {
            CFDictionaryGetValueIfPresent(
                dict_ref,
                k_window_number.as_concrete_TypeRef() as *const c_void,
                &mut value,
            )
        };
        if found == 0 || value.is_null() {
            continue;
        }
        let number = unsafe { CFNumber::wrap_under_get_rule(value as CFTypeRef as *const _) };
        if number.to_i64() != Some(i64::from(native_window_id)) {
            continue;
        }

        value = std::ptr::null();
        let found = unsafe {
            CFDictionaryGetValueIfPresent(
                dict_ref,
                k_owner_pid.as_concrete_TypeRef() as *const c_void,
                &mut value,
            )
        };
        if found == 0 || value.is_null() {
            continue;
        }
        let owner = unsafe { CFNumber::wrap_under_get_rule(value as CFTypeRef as *const _) };
        if let Some(pid) = owner.to_i64().and_then(|pid| i32::try_from(pid).ok()) {
            owner_pids.push(pid);
        }
    }

    owner_pids.sort_unstable();
    owner_pids.dedup();

    match owner_pids.as_slice() {
        [] => Err(NativeWindowCaptureError::NativeWindowNotFound { native_window_id }),
        [pid] => Ok(*pid),
        _ => Err(NativeWindowCaptureError::AmbiguousNativeWindowId {
            native_window_id,
            count: owner_pids.len(),
        }),
    }
}

#[cfg(not(target_os = "macos"))]
fn core_graphics_owner_pid_for_native_window(
    native_window_id: u32,
) -> Result<i32, NativeWindowCaptureError> {
    Err(NativeWindowCaptureError::NativeWindowNotFound { native_window_id })
}

// ── Shared candidate-selection infrastructure ───────────────────────────

const SCRIPT_KIT_CANDIDATE_MIN_WIDTH: u32 = 200;
const SCRIPT_KIT_CANDIDATE_MIN_HEIGHT: u32 = 50;
const CANDIDATE_SIZE_CLOSE_DELTA_PX: i32 = 4;
const CANDIDATE_SIZE_LOOSE_DELTA_PX: i32 = 16;
const CANDIDATE_SIZE_EXACT_MATCH_SCORE: i32 = 5_000;
const CANDIDATE_SIZE_CLOSE_MATCH_SCORE: i32 = 2_500;
const CANDIDATE_SIZE_LOOSE_MATCH_SCORE: i32 = 500;
const CANDIDATE_SIZE_MISMATCH_PENALTY: i32 = -1_500;
const CANDIDATE_TITLE_EXACT_MATCH_SCORE: i32 = 1_000;
const CANDIDATE_TITLE_CONTAINS_MATCH_SCORE: i32 = 500;
const CANDIDATE_FOCUS_MATCH_SCORE: i32 = 100;
const MAIN_WINDOW_SECONDARY_TITLE_PENALTY: i32 = 200;

#[derive(Clone)]
struct Candidate {
    window: Window,
    title: String,
    app_name: String,
    focused: bool,
    width: i32,
    height: i32,
}

/// Enumerate all visible Script Kit OS windows that are large enough to be
/// meaningful screenshot targets.
fn list_script_kit_candidates() -> Result<Vec<Candidate>, Box<dyn std::error::Error + Send + Sync>>
{
    list_script_kit_candidates_for(None)
}

/// Like [`list_script_kit_candidates`], but when the resolved automation
/// target itself declares bounds smaller than the default size floor (e.g.
/// the ~36px-tall footer overlay strip), admit candidates down to that
/// declared size so registered small windows stay capturable without
/// re-opening the filter to arbitrary tiny UI elements.
fn list_script_kit_candidates_for(
    resolved: Option<&crate::protocol::AutomationWindowInfo>,
) -> Result<Vec<Candidate>, Box<dyn std::error::Error + Send + Sync>> {
    let (min_width, min_height) = match resolved.and_then(|info| info.bounds.as_ref()) {
        Some(bounds) => (
            SCRIPT_KIT_CANDIDATE_MIN_WIDTH.min(bounds.width.round().max(1.0) as u32),
            SCRIPT_KIT_CANDIDATE_MIN_HEIGHT.min(bounds.height.round().max(1.0) as u32),
        ),
        None => (
            SCRIPT_KIT_CANDIDATE_MIN_WIDTH,
            SCRIPT_KIT_CANDIDATE_MIN_HEIGHT,
        ),
    };

    let mut candidates = Vec::new();
    for window in Window::all()? {
        let title = window.title().unwrap_or_else(|_| String::new());
        let app_name = window.app_name().unwrap_or_else(|_| String::new());
        let focused = window.is_focused().unwrap_or(false);
        let is_minimized = window.is_minimized().unwrap_or(true);
        let width = window.width().unwrap_or(0);
        let height = window.height().unwrap_or(0);

        let is_our_window = app_name.contains("script-kit-gpui")
            || app_name == "Script Kit"
            || title.contains("Script Kit");

        // Width >= 200 filters out small UI elements
        // Height >= 50 allows compact prompts (arg prompt without choices is ~76px)
        let is_reasonable_size = width >= min_width && height >= min_height;

        if is_our_window && !is_minimized && is_reasonable_size {
            candidates.push(Candidate {
                window,
                title,
                app_name,
                focused,
                width: width as i32,
                height: height as i32,
            });
        }
    }
    Ok(candidates)
}

/// Score how well a candidate's dimensions match the resolved target's bounds.
///
/// Returns a bonus/penalty based on dimension proximity. Exact match gets the
/// highest bonus; large deviations get a penalty to push mismatched windows
/// below better candidates.
fn candidate_size_score(
    resolved: &crate::protocol::AutomationWindowInfo,
    candidate: &Candidate,
) -> i32 {
    let Some(bounds) = resolved.bounds.as_ref() else {
        return 0;
    };

    let target_w = bounds.width.round() as i32;
    let target_h = bounds.height.round() as i32;

    let dw = (candidate.width - target_w).abs();
    let dh = (candidate.height - target_h).abs();

    match (dw, dh) {
        (0, 0) => CANDIDATE_SIZE_EXACT_MATCH_SCORE,
        (dw, dh) if dw <= CANDIDATE_SIZE_CLOSE_DELTA_PX && dh <= CANDIDATE_SIZE_CLOSE_DELTA_PX => {
            CANDIDATE_SIZE_CLOSE_MATCH_SCORE
        }
        (dw, dh) if dw <= CANDIDATE_SIZE_LOOSE_DELTA_PX && dh <= CANDIDATE_SIZE_LOOSE_DELTA_PX => {
            CANDIDATE_SIZE_LOOSE_MATCH_SCORE
        }
        _ => CANDIDATE_SIZE_MISMATCH_PENALTY,
    }
}

/// Score an OS window candidate against a resolved automation target.
///
/// Higher scores mean a better match. Uses bounds as a first-class signal
/// when available, plus title and focus agreement. The resolved target's
/// metadata drives selection.
fn score_candidate(resolved: &crate::protocol::AutomationWindowInfo, candidate: &Candidate) -> i32 {
    use crate::protocol::AutomationWindowKind;

    let mut score: i32 = candidate_size_score(resolved, candidate);

    // Exact title match is a strong signal
    if let Some(title) = resolved.title.as_deref() {
        if !title.is_empty() && candidate.title == title {
            score += CANDIDATE_TITLE_EXACT_MATCH_SCORE;
        } else if !title.is_empty() && candidate.title.contains(title) {
            score += CANDIDATE_TITLE_CONTAINS_MATCH_SCORE;
        }
    }

    // Focus agreement
    if resolved.focused == candidate.focused {
        score += CANDIDATE_FOCUS_MATCH_SCORE;
    }

    // For main-window targets, penalize candidates that are clearly secondary
    // windows (Notes, AI) so we never accidentally prefer them.
    if resolved.kind == AutomationWindowKind::Main
        && (candidate.title.contains("Notes")
            || candidate.title.contains("AI")
            || candidate.title.contains("Agent Chat"))
    {
        score -= MAIN_WINDOW_SECONDARY_TITLE_PENALTY;
    }

    score
}

/// Select the best-matching OS window candidate for a resolved automation target.
///
/// Returns a hard error when no OS window matches or when the top two
/// candidates tie (ambiguous match). Emits structured logs on every
/// successful selection and on ambiguous rejection so agents can audit
/// which OS window was actually selected.
///
/// This is the single candidate-ranking path reused by PNG capture,
/// RGBA capture, and native `os_window_id` resolution.
fn select_best_candidate<'a>(
    resolved: &crate::protocol::AutomationWindowInfo,
    candidates: &'a [Candidate],
    caller: &str,
) -> Result<&'a Candidate, Box<dyn std::error::Error + Send + Sync>> {
    let mut ranked: Vec<(i32, &Candidate)> = candidates
        .iter()
        .map(|candidate| (score_candidate(resolved, candidate), candidate))
        .collect();

    ranked.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| right.focused.cmp(&left.focused))
            .then_with(|| right.title.cmp(&left.title))
    });

    let Some((best_score, best)) = ranked.first().copied() else {
        return Err(
            "No visible Script Kit windows available for screenshot capture"
                .to_string()
                .into(),
        );
    };

    if best_score <= 0 {
        return Err(format!(
            "No OS window matched automation target {} ({:?}) \
             strongly enough for deterministic capture",
            resolved.id, resolved.kind
        )
        .into());
    }

    // Reject tied top candidates instead of guessing
    if let Some((second_score, second)) = ranked.get(1).copied() {
        if second_score == best_score {
            tracing::warn!(
                target: "script_kit::automation",
                window_id = %resolved.id,
                kind = ?resolved.kind,
                caller = %caller,
                first_title = %best.title,
                first_size = %format!("{}x{}", best.width, best.height),
                second_title = %second.title,
                second_size = %format!("{}x{}", second.width, second.height),
                score = best_score,
                "automation.capture_screenshot.ambiguous_candidate"
            );

            return Err(format!(
                "Ambiguous OS window match for automation target {} ({:?}); \
                 '{}' and '{}' tied at score {}",
                resolved.id, resolved.kind, best.title, second.title, best_score
            )
            .into());
        }
    }

    tracing::info!(
        target: "script_kit::automation",
        window_id = %resolved.id,
        kind = ?resolved.kind,
        caller = %caller,
        requested_title = ?resolved.title,
        requested_bounds = ?resolved.bounds,
        candidate_count = ranked.len(),
        selected_title = %best.title,
        selected_app = %best.app_name,
        selected_focused = best.focused,
        selected_width = best.width,
        selected_height = best.height,
        selected_score = best_score,
        "automation.capture_screenshot.candidate_selected"
    );

    Ok(best)
}

/// Capture the OS window that best matches the resolved automation target.
///
/// Delegates to `select_best_candidate` for the single shared ranking path.
fn capture_resolved_window(
    resolved: &crate::protocol::AutomationWindowInfo,
    hi_dpi: bool,
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    let candidates = list_script_kit_candidates_for(Some(resolved))?;
    let best = select_best_candidate(resolved, &candidates, "capture_png")?;
    capture_and_encode_png(&best.window, hi_dpi)
}

// ── Popup capture receipt ──────────────────────────────────────────────

/// Strategy used to capture a popup window for verification.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopupCaptureStrategy {
    /// Captured the parent window and crop to popup bounds.
    ParentCaptureWithCrop,
    /// Captured the detached window directly.
    DirectWindowCapture,
    /// Not a popup target (main window or unknown kind).
    NotApplicable,
}

/// Deterministic receipt for popup screenshot capture.
///
/// Always included in targeted capture results so agents can distinguish
/// how a popup was captured and whether the crop is trustworthy.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupCaptureReceipt {
    /// The capture strategy used.
    pub strategy: PopupCaptureStrategy,
    /// The automation window kind that determined the strategy.
    pub window_kind: String,
    /// Crop bounds within the screenshot (null for detached or when not applicable).
    pub target_bounds: Option<crate::protocol::InspectBoundsInScreenshot>,
    /// Whether semantic receipts (not screenshots) are the primary verification oracle.
    pub semantic_receipts_are_primary: bool,
}

/// Returns `true` for window kinds that are attached popups (rendered
/// inside the parent window's coordinate space).
fn is_attached_popup(kind: crate::protocol::AutomationWindowKind) -> bool {
    matches!(
        kind,
        crate::protocol::AutomationWindowKind::ActionsDialog
            | crate::protocol::AutomationWindowKind::PromptPopup
    )
}

/// Returns `true` for window kinds that are detached (own OS window).
fn is_detached_window(kind: crate::protocol::AutomationWindowKind) -> bool {
    matches!(
        kind,
        crate::protocol::AutomationWindowKind::AgentChatDetached
            | crate::protocol::AutomationWindowKind::Dictation
            | crate::protocol::AutomationWindowKind::Notes
            | crate::protocol::AutomationWindowKind::Hud
    )
}

/// Capture a screenshot of a targeted window and produce a popup capture receipt.
///
/// For **attached popups** (ActionsDialog, PromptPopup):
/// - Resolves the parent window identity and popup bounds.
/// - Captures the parent window and computes crop bounds.
/// - Emits `automation.capture_screenshot.parent_crop_selected` on success.
/// - **Fails loudly** with `automation.capture_screenshot.parent_crop_failed`
///   if parent identity or popup bounds are missing — never returns an
///   unscoped whole-window screenshot.
///
/// For **detached windows** (AgentChatDetached, Notes):
/// - Captures the window directly.
///
/// For **other targets** (Main, etc.):
/// - Standard capture with `not_applicable` strategy.
#[allow(clippy::type_complexity)]
pub fn capture_targeted_screenshot_with_popup_receipt(
    target: Option<&crate::protocol::AutomationWindowTarget>,
    hi_dpi: bool,
) -> Result<(Vec<u8>, u32, u32, PopupCaptureReceipt), Box<dyn std::error::Error + Send + Sync>> {
    let resolved = match crate::windows::resolve_automation_window(target) {
        Ok(info) => info,
        Err(err) => {
            tracing::warn!(
                target: "script_kit::automation",
                error = %err,
                target = ?target,
                "automation.capture_screenshot.target_failed"
            );
            return Err(err.to_string().into());
        }
    };

    let kind = resolved.kind;
    let kind_str = format!("{kind:?}");

    if is_attached_popup(kind) {
        // Attached popup: require parent identity and bounds.
        let parent_id = match resolved.parent_window_id.as_ref() {
            Some(pid) => pid.clone(),
            None => {
                tracing::error!(
                    target: "script_kit::automation",
                    window_id = %resolved.id,
                    kind = %kind_str,
                    "automation.capture_screenshot.parent_crop_failed: \
                     attached popup has no parent_window_id in automation registry"
                );
                return Err(format!(
                    "Attached popup '{}' ({}) cannot be captured: \
                     no parent window identity registered. \
                     Refusing to fall back to unscoped whole-window screenshot.",
                    resolved.id, kind_str
                )
                .into());
            }
        };

        // Resolve parent window metadata
        let parent_target = crate::protocol::AutomationWindowTarget::Id {
            id: parent_id.clone(),
        };
        let parent_resolved = match crate::windows::resolve_automation_window(Some(&parent_target))
        {
            Ok(p) => p,
            Err(err) => {
                tracing::error!(
                    target: "script_kit::automation",
                    window_id = %resolved.id,
                    kind = %kind_str,
                    parent_id = %parent_id,
                    error = %err,
                    "automation.capture_screenshot.parent_crop_failed: \
                     parent window not found in automation registry"
                );
                return Err(format!(
                    "Attached popup '{}' ({}) cannot be captured: \
                     parent '{}' not found in registry. \
                     Refusing to fall back to unscoped whole-window screenshot.",
                    resolved.id, kind_str, parent_id
                )
                .into());
            }
        };

        // Compute crop bounds
        let target_bounds = crate::protocol::target_bounds_in_screenshot_with_main(
            &resolved,
            parent_resolved.bounds.as_ref(),
        );

        if target_bounds.is_none() {
            tracing::error!(
                target: "script_kit::automation",
                window_id = %resolved.id,
                kind = %kind_str,
                parent_id = %parent_id,
                popup_bounds = ?resolved.bounds,
                parent_bounds = ?parent_resolved.bounds,
                "automation.capture_screenshot.parent_crop_failed: \
                 popup or parent bounds missing, cannot compute crop region"
            );
            return Err(format!(
                "Attached popup '{}' ({}) cannot be captured: \
                 bounds geometry unavailable (popup bounds: {}, parent bounds: {}). \
                 Refusing to produce unscoped whole-window screenshot.",
                resolved.id,
                kind_str,
                if resolved.bounds.is_some() {
                    "present"
                } else {
                    "missing"
                },
                if parent_resolved.bounds.is_some() {
                    "present"
                } else {
                    "missing"
                },
            )
            .into());
        }

        // Capture the parent window
        tracing::info!(
            target: "script_kit::automation",
            window_id = %resolved.id,
            kind = %kind_str,
            parent_id = %parent_id,
            hi_dpi = hi_dpi,
            "automation.capture_screenshot.targeted"
        );

        let (png_data, width, height) = capture_resolved_window(&parent_resolved, hi_dpi)?;

        tracing::info!(
            target: "script_kit::automation",
            window_id = %resolved.id,
            kind = %kind_str,
            parent_id = %parent_id,
            parent_width = width,
            parent_height = height,
            crop_bounds = ?target_bounds,
            "automation.capture_screenshot.parent_crop_selected"
        );

        let receipt = PopupCaptureReceipt {
            strategy: PopupCaptureStrategy::ParentCaptureWithCrop,
            window_kind: kind_str,
            target_bounds,
            semantic_receipts_are_primary: true,
        };

        Ok((png_data, width, height, receipt))
    } else if is_detached_window(kind) {
        // Detached window: direct capture
        tracing::info!(
            target: "script_kit::automation",
            window_id = %resolved.id,
            kind = %kind_str,
            hi_dpi = hi_dpi,
            "automation.capture_screenshot.targeted"
        );

        let (png_data, width, height) = capture_resolved_window(&resolved, hi_dpi)?;

        let receipt = PopupCaptureReceipt {
            strategy: PopupCaptureStrategy::DirectWindowCapture,
            window_kind: kind_str,
            target_bounds: None,
            semantic_receipts_are_primary: true,
        };

        Ok((png_data, width, height, receipt))
    } else {
        // Main or other: standard capture
        tracing::info!(
            target: "script_kit::automation",
            window_id = %resolved.id,
            kind = %kind_str,
            hi_dpi = hi_dpi,
            "automation.capture_screenshot.targeted"
        );

        let (png_data, width, height) = capture_resolved_window(&resolved, hi_dpi)?;

        let receipt = PopupCaptureReceipt {
            strategy: PopupCaptureStrategy::NotApplicable,
            window_kind: kind_str,
            target_bounds: None,
            semantic_receipts_are_primary: false,
        };

        Ok((png_data, width, height, receipt))
    }
}

// ── Public API ──────────────────────────────────────────────────────────

/// Capture a screenshot of the main app window.
///
/// Resolves the main automation target first, then uses the shared
/// candidate-selection path. Does **not** preferentially capture Notes or
/// AI windows by heuristic — the resolved target drives selection.
///
/// # Arguments
/// * `hi_dpi` - If true, return full retina resolution (2x). If false, scale down to 1x.
pub fn capture_app_screenshot(
    hi_dpi: bool,
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    let target = crate::protocol::AutomationWindowTarget::Main;
    let resolved =
        crate::windows::resolve_automation_window(Some(&target)).map_err(|err| err.to_string())?;
    capture_resolved_window(&resolved, hi_dpi)
}

/// Capture a screenshot of a window by its title pattern.
///
/// Similar to `capture_app_screenshot` but allows specifying which window to capture
/// by matching the title. This is useful for secondary windows like the Agent Chat Chat window.
///
/// # Arguments
/// * `title_pattern` - A string that the window title must contain
/// * `hi_dpi` - If true, return full retina resolution (2x). If false, scale down to 1x.
///
/// # Returns
/// A tuple of (png_data, width, height) on success.
const TITLE_CAPTURE_MIN_WINDOW_EDGE: u32 = 100;

pub fn capture_window_by_title(
    title_pattern: &str,
    hi_dpi: bool,
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    let windows = Window::all()?;

    for window in windows {
        let title = window.title().unwrap_or_else(|_| String::new());
        let app_name = window.app_name().unwrap_or_else(|_| String::new());

        // Match window by title pattern (must also be our app)
        let is_our_app = app_name.contains("script-kit-gpui") || app_name == "Script Kit";
        let title_matches = title.contains(title_pattern);
        let is_minimized = window.is_minimized().unwrap_or(true);
        // Skip tiny windows (e.g. tray icon) when using empty title pattern
        let win_width = window.width().unwrap_or(0);
        let win_height = window.height().unwrap_or(0);
        let is_too_small = win_width < TITLE_CAPTURE_MIN_WINDOW_EDGE || win_height < TITLE_CAPTURE_MIN_WINDOW_EDGE;

        if is_our_app && title_matches && !is_minimized && !is_too_small {
            tracing::debug!(
                app_name = %app_name,
                title = %title,
                title_pattern = %title_pattern,
                hi_dpi = hi_dpi,
                "Found window matching title pattern for screenshot"
            );

            let (png_data, width, height) = capture_and_encode_png(&window, hi_dpi)?;

            tracing::debug!(
                width = width,
                height = height,
                hi_dpi = hi_dpi,
                file_size = png_data.len(),
                title_pattern = %title_pattern,
                "Screenshot captured for window by title"
            );

            return Ok((png_data, width, height));
        }
    }

    Err(format!("Window with title containing '{}' not found", title_pattern).into())
}

/// Capture a screenshot routed through the automation window target resolver.
///
/// Always captures through the resolved `AutomationWindowInfo` metadata
/// using the shared candidate-selection path. Returns a hard error when
/// no OS window matches the resolved target — never silently falls back
/// to the main window.
///
/// # Arguments
/// * `target` - The automation window target. `None` defaults to `Focused`.
/// * `hi_dpi` - If true, return full retina resolution (2x). If false, scale down to 1x.
pub fn capture_targeted_screenshot(
    target: Option<&crate::protocol::AutomationWindowTarget>,
    hi_dpi: bool,
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    let resolved = match crate::windows::resolve_automation_window(target) {
        Ok(info) => info,
        Err(err) => {
            tracing::warn!(
                target: "script_kit::automation",
                error = %err,
                target = ?target,
                "automation.capture_screenshot.target_failed"
            );
            return Err(err.to_string().into());
        }
    };

    tracing::info!(
        target: "script_kit::automation",
        window_id = %resolved.id,
        kind = ?resolved.kind,
        hi_dpi = hi_dpi,
        "automation.capture_screenshot.targeted"
    );

    capture_resolved_window(&resolved, hi_dpi)
}

/// Capture a window screenshot using the resolver-driven path, translating a title
/// pattern into an `AutomationWindowTarget` for deterministic capture.
///
/// This is the replacement for direct `capture_window_by_title` calls from stdin/runtime
/// paths. Empty titles resolve to `AutomationWindowTarget::Main`; non-empty titles
/// resolve to `AutomationWindowTarget::TitleContains`.
///
/// Emits an explicit compatibility log (`automation.capture_screenshot.title_compatibility`)
/// before delegating to `capture_targeted_screenshot`.
pub fn capture_window_by_title_via_resolver(
    title_pattern: &str,
    hi_dpi: bool,
) -> Result<(Vec<u8>, u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    let target = if title_pattern.trim().is_empty() {
        crate::protocol::AutomationWindowTarget::Main
    } else {
        crate::protocol::AutomationWindowTarget::TitleContains {
            text: title_pattern.to_string(),
        }
    };

    tracing::info!(
        target: "script_kit::automation",
        title_pattern = %title_pattern,
        resolved_target = ?target,
        "automation.capture_screenshot.title_compatibility"
    );

    capture_targeted_screenshot(Some(&target), hi_dpi)
}

/// Capture a raw RGBA image of the OS window matching the resolved automation target.
///
/// Returns the raw `image::RgbaImage` (not PNG-encoded) so callers can
/// extract dimensions and sample individual pixels without a decode step.
/// Used by `inspectAutomationWindow` for lightweight pixel probes.
///
/// Delegates to `select_best_candidate` for the single shared ranking path,
/// emitting the same ambiguity/selection logs as PNG capture and OS window ID
/// resolution.
pub fn capture_targeted_rgba_image(
    target: Option<&crate::protocol::AutomationWindowTarget>,
    hi_dpi: bool,
) -> Result<image::RgbaImage, Box<dyn std::error::Error + Send + Sync>> {
    const DOWNSCALE_DIVISOR: u32 = 2;

    let resolved =
        crate::windows::resolve_automation_window(target).map_err(|err| err.to_string())?;

    let candidates = list_script_kit_candidates_for(Some(&resolved))?;
    let best = select_best_candidate(&resolved, &candidates, "capture_rgba")?;

    let rgba_image = best.window.capture_image()?;

    if hi_dpi {
        Ok(rgba_image)
    } else {
        let new_width = (rgba_image.width() / DOWNSCALE_DIVISOR).max(1);
        let new_height = (rgba_image.height() / DOWNSCALE_DIVISOR).max(1);
        Ok(image::imageops::resize(
            &rgba_image,
            new_width,
            new_height,
            image::imageops::FilterType::Lanczos3,
        ))
    }
}

/// Resolve the best-matching OS window for an automation target and return its
/// native CGWindowID.
///
/// Delegates to `select_best_candidate` for the single shared ranking path,
/// emitting the same ambiguity/selection logs as PNG and RGBA capture.
/// Returns `None` when no candidate matches strongly enough.
pub fn resolve_targeted_os_window_id(
    target: Option<&crate::protocol::AutomationWindowTarget>,
) -> Option<u32> {
    let resolved = crate::windows::resolve_automation_window(target).ok()?;
    let candidates = list_script_kit_candidates_for(Some(&resolved)).ok()?;
    let best = select_best_candidate(&resolved, &candidates, "resolve_os_window_id").ok()?;
    best.window.id().ok()
}

#[cfg(test)]
mod screenshots_window_open_tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    #[test]
    fn pixel_audit_rejects_opaque_black_image() {
        let image = ImageBuffer::from_pixel(8, 8, Rgba([0, 0, 0, 255]));
        let audit = audit_screenshot_pixels(&image);

        assert_eq!(audit.sampled, 64);
        assert_eq!(audit.non_transparent, 64);
        assert_eq!(audit.non_black, 0);
        assert_eq!(audit.max_luma, 0.0);
        assert_eq!(audit.non_black_ratio, 0.0);
        assert!(audit.is_blank_like());
    }

    #[test]
    fn pixel_audit_rejects_opaque_white_image() {
        let image = ImageBuffer::from_pixel(8, 8, Rgba([255, 255, 255, 255]));
        let audit = audit_screenshot_pixels(&image);

        assert_eq!(audit.unique_bucket_count, 1);
        assert_eq!(audit.non_black, 64);
        assert_eq!(audit.non_black_ratio, 1.0);
        assert!(audit.is_blank_like());
    }

    #[test]
    fn pixel_audit_rejects_transparent_image() {
        let image = ImageBuffer::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        let audit = audit_screenshot_pixels(&image);

        assert_eq!(audit.non_transparent, 0);
        assert!(audit.is_blank_like());
    }

    #[test]
    fn pixel_audit_accepts_valid_dark_ui_pixels() {
        let mut image = ImageBuffer::from_pixel(16, 16, Rgba([0, 0, 0, 255]));
        for x in 4..12 {
            image.put_pixel(x, 8, Rgba([40, 40, 40, 255]));
        }

        let audit = audit_screenshot_pixels(&image);

        assert!(audit.unique_bucket_count <= 2);
        assert!(audit.mean_luma < 5.0);
        assert!(audit.non_black_ratio >= 0.001);
        assert!(audit.max_luma >= 16.0);
        assert!(!audit.is_blank_like());
    }

    #[test]
    fn pixel_audit_accepts_visible_ui_pixels() {
        let mut image = ImageBuffer::from_pixel(8, 8, Rgba([0, 0, 0, 255]));
        for x in 2..6 {
            for y in 2..6 {
                image.put_pixel(x, y, Rgba([240, 240, 240, 255]));
            }
        }

        let audit = audit_screenshot_pixels(&image);

        assert!(audit.non_black > 0);
        assert!(!audit.is_blank_like());
    }
}

//! Native policy for the explicitly owned, non-presenting evaluation host.
//! This is separate from application effect policy and never enables itself from environment.
use crate::{Bounds, Pixels, WindowKind, WindowParams};
use anyhow::{Result, ensure};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicU64, Ordering},
};

static INSTALLED: OnceLock<Arc<OwnedHiddenGuard>> = OnceLock::new();

/// Hard bounds shared by native allocation and completed-scene readback.
pub const OWNED_HIDDEN_MAX_PIXELS: u64 = 4_194_304;
/// Maximum simultaneously live native evaluation windows.
pub const OWNED_HIDDEN_MAX_WINDOWS: u64 = 8;
/// Bound encoded image/SVG data using the existing RGBA pixel allocation ceiling.
pub const OWNED_HIDDEN_MAX_RESOURCE_BYTES: u64 = OWNED_HIDDEN_MAX_PIXELS * 4;
/// Bound retained animation work independently of encoded compression ratio.
pub const OWNED_HIDDEN_MAX_IMAGE_FRAMES: usize = 128;

/// Negative-only one-shot corruption at the native owned readback boundary.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnedReadbackFault {
    /// Zero the actual image returned by Metal; these are explicitly faulted pixels.
    Blank,
    /// Refuse the next readback instead of returning an image.
    Failure,
}

/// Observations of the native guard, not a claim about application effects.
#[derive(Clone, Copy, Debug, Default)]
pub struct OwnedHiddenObservation {
    /// Whether the process-wide native guard has been installed.
    pub installed: bool,
    /// Total native windows created under this guard.
    pub opened_windows: u64,
    /// Native windows not yet destroyed.
    pub live_windows: u64,
    /// Native frame draws completed under this guard.
    pub completed_frames: u64,
    /// Completed native images read back under this guard.
    pub readback_images: u64,
    /// Native operations rejected before their external effect.
    pub refused_operations: u64,
}

/// Process-wide native authority, installed before any normal MacPlatform initialization.
#[derive(Default)]
pub struct OwnedHiddenGuard {
    opened_windows: AtomicU64,
    live_windows: AtomicU64,
    completed_frames: AtomicU64,
    readback_images: AtomicU64,
    refused_operations: AtomicU64,
    resource_path_validator: Option<Arc<dyn Fn(&std::path::Path) -> Result<()> + Send + Sync>>,
}

impl OwnedHiddenGuard {
    /// Install once; an already initialized evaluator cannot be replaced.
    pub fn install(
        resource_path_validator: Arc<dyn Fn(&std::path::Path) -> Result<()> + Send + Sync>,
    ) -> Result<Arc<Self>> {
        let guard = Arc::new(Self {
            resource_path_validator: Some(resource_path_validator),
            ..Self::default()
        });
        INSTALLED
            .set(guard.clone())
            .map_err(|_| anyhow::anyhow!("owned_hidden_already_installed"))?;
        Ok(guard)
    }

    /// Current native policy, including for otherwise unguarded native entry points.
    pub fn installed() -> Option<&'static Arc<Self>> {
        INSTALLED.get()
    }

    /// Delegate path authorization to the application's one installed owned-root authority.
    pub(crate) fn read_resource_path(&self, path: &std::path::Path) -> Result<Vec<u8>> {
        use std::io::Read as _;
        let validate = self
            .resource_path_validator
            .as_ref()
            .ok_or_else(|| self.refuse("resource_path_validator_missing"))?;
        validate(path).inspect_err(|_| {
            self.refused_operations.fetch_add(1, Ordering::Relaxed);
        })?;
        let metadata = std::fs::metadata(path)?;
        ensure!(metadata.is_file(), "owned_resource_not_regular_file");
        ensure!(
            metadata.len() <= OWNED_HIDDEN_MAX_RESOURCE_BYTES,
            "owned_resource_byte_limit"
        );
        let file = std::fs::File::open(path)?;
        let mut bytes = Vec::new();
        file.take(OWNED_HIDDEN_MAX_RESOURCE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= OWNED_HIDDEN_MAX_RESOURCE_BYTES,
            "owned_resource_byte_limit"
        );
        Ok(bytes)
    }

    /// Record a refusal. Void platform methods cannot return an error; their refusal is observable.
    pub fn refuse(&self, operation: &str) -> anyhow::Error {
        self.refused_operations.fetch_add(1, Ordering::Relaxed);
        anyhow::anyhow!("owned_hidden_forbidden:{operation}")
    }

    /// Validate the final parameters immediately before native allocation.
    pub fn validate_window(&self, params: &WindowParams) -> Result<()> {
        validate_owned_hidden_params(params).inspect_err(|_| {
            self.refused_operations.fetch_add(1, Ordering::Relaxed);
        })
    }

    /// Reserve one actual native lifetime after validation and before allocation.
    pub fn window_opened(&self) -> Result<()> {
        self.live_windows
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < OWNED_HIDDEN_MAX_WINDOWS).then_some(n + 1)
            })
            .map_err(|_| self.refuse("window_limit"))?;
        self.opened_windows.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Called after the exact native lifetime has closed.
    pub fn window_closed(&self) {
        self.live_windows.fetch_sub(1, Ordering::AcqRel);
    }
    /// A GPUI layout/paint transaction completed; this is not presentation.
    pub fn frame_completed(&self) {
        self.completed_frames.fetch_add(1, Ordering::Relaxed);
    }
    /// A real offscreen Metal readback completed successfully.
    pub fn image_completed(&self) {
        self.readback_images.fetch_add(1, Ordering::Relaxed);
    }

    /// Capture cumulative native observations.
    pub fn observation(&self) -> OwnedHiddenObservation {
        OwnedHiddenObservation {
            installed: Self::installed()
                .is_some_and(|installed| std::ptr::eq(self, installed.as_ref())),
            opened_windows: self.opened_windows.load(Ordering::Acquire),
            live_windows: self.live_windows.load(Ordering::Acquire),
            completed_frames: self.completed_frames.load(Ordering::Acquire),
            readback_images: self.readback_images.load(Ordering::Acquire),
            refused_operations: self.refused_operations.load(Ordering::Acquire),
        }
    }
}

/// Bounds must be finite, positive, and fit the maximum image at the actual scale.
pub fn validate_owned_hidden_bounds(bounds: Bounds<Pixels>, scale: f32) -> Result<()> {
    let values = [
        bounds.origin.x.as_f32(),
        bounds.origin.y.as_f32(),
        bounds.size.width.as_f32(),
        bounds.size.height.as_f32(),
        scale,
    ];
    ensure!(
        values.iter().all(|value| value.is_finite()),
        "invalid_hidden_bounds"
    );
    ensure!(
        values[2] > 0.0 && values[3] > 0.0 && scale > 0.0,
        "invalid_hidden_size"
    );
    let width = (f64::from(values[2]) * f64::from(scale)).ceil();
    let height = (f64::from(values[3]) * f64::from(scale)).ceil();
    ensure!(
        width * height <= OWNED_HIDDEN_MAX_PIXELS as f64,
        "owned_hidden_pixel_limit"
    );
    Ok(())
}

/// Reject unsafe requests rather than silently rewriting them into hidden windows.
pub fn validate_owned_hidden_params(params: &WindowParams) -> Result<()> {
    ensure!(!params.show && !params.focus, "owned_hidden_show_or_focus");
    ensure!(
        matches!(params.kind, WindowKind::PopUp),
        "owned_hidden_window_kind"
    );
    ensure!(
        !params.is_movable && !params.is_resizable && !params.is_minimizable,
        "owned_hidden_native_interaction"
    );
    #[cfg(target_os = "macos")]
    ensure!(params.tabbing_identifier.is_none(), "owned_hidden_tabbing");
    validate_owned_hidden_bounds(params.bounds, 1.0)
}

pub(crate) fn validate_owned_image_size(width: u32, height: u32) -> Result<u64> {
    let pixels = u64::from(width) * u64::from(height);
    ensure!(
        width > 0 && height > 0 && pixels <= OWNED_HIDDEN_MAX_PIXELS,
        "owned_image_pixel_limit"
    );
    Ok(pixels)
}

pub(crate) fn owned_image_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(OWNED_HIDDEN_MAX_RESOURCE_BYTES);
    limits
}

pub(crate) fn decode_owned_image(
    bytes: &[u8],
    format: image::ImageFormat,
) -> Result<smallvec::SmallVec<[image::Frame; 1]>> {
    use image::{AnimationDecoder as _, ImageDecoder as _};
    use std::io::Cursor;
    ensure!(
        bytes.len() as u64 <= OWNED_HIDDEN_MAX_RESOURCE_BYTES,
        "owned_resource_byte_limit"
    );
    fn frames(iter: image::Frames<'_>) -> Result<smallvec::SmallVec<[image::Frame; 1]>> {
        let mut result = smallvec::SmallVec::new();
        let mut pixels = 0_u64;
        for frame in iter {
            ensure!(
                result.len() < OWNED_HIDDEN_MAX_IMAGE_FRAMES,
                "owned_image_frame_limit"
            );
            let mut frame = frame?;
            pixels = pixels
                .checked_add(validate_owned_image_size(
                    frame.buffer().width(),
                    frame.buffer().height(),
                )?)
                .ok_or_else(|| anyhow::anyhow!("owned_image_pixel_limit"))?;
            ensure!(pixels <= OWNED_HIDDEN_MAX_PIXELS, "owned_image_pixel_limit");
            for pixel in frame.buffer_mut().chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            result.push(frame);
        }
        ensure!(!result.is_empty(), "owned_image_empty");
        Ok(result)
    }
    match format {
        image::ImageFormat::Gif => {
            let mut decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))?;
            let (width, height) = decoder.dimensions();
            validate_owned_image_size(width, height)?;
            decoder.set_limits(owned_image_limits())?;
            frames(decoder.into_frames())
        }
        image::ImageFormat::WebP => {
            let mut decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))?;
            let (width, height) = decoder.dimensions();
            validate_owned_image_size(width, height)?;
            decoder.set_limits(owned_image_limits())?;
            if decoder.has_animation() {
                let _ = decoder.set_background_color(image::Rgba([0, 0, 0, 0]));
                frames(decoder.into_frames())
            } else {
                let mut image = image::DynamicImage::from_decoder(decoder)?.into_rgba8();
                for pixel in image.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
                Ok(smallvec::smallvec![image::Frame::new(image)])
            }
        }
        _ => {
            let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
            reader.limits(owned_image_limits());
            let decoder = reader.into_decoder()?;
            let (width, height) = decoder.dimensions();
            validate_owned_image_size(width, height)?;
            let mut image = image::DynamicImage::from_decoder(decoder)?.into_rgba8();
            for pixel in image.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            Ok(smallvec::smallvec![image::Frame::new(image)])
        }
    }
}

pub(crate) fn owned_svg_bytes(bytes: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>> {
    use std::io::Read as _;
    ensure!(
        bytes.len() as u64 <= OWNED_HIDDEN_MAX_RESOURCE_BYTES,
        "owned_resource_byte_limit"
    );
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return Ok(std::borrow::Cow::Borrowed(bytes));
    }
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut decoded = Vec::new();
    decoder
        .take(OWNED_HIDDEN_MAX_RESOURCE_BYTES + 1)
        .read_to_end(&mut decoded)?;
    ensure!(
        decoded.len() as u64 <= OWNED_HIDDEN_MAX_RESOURCE_BYTES,
        "owned_svg_decompression_limit"
    );
    Ok(std::borrow::Cow::Owned(decoded))
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn uninstalled_guard_never_claims_process_authority() {
        let guard = OwnedHiddenGuard::default();
        assert!(!guard.observation().installed);
    }

    #[test]
    fn resource_path_authorization_precedes_filesystem_io() {
        let guard = OwnedHiddenGuard {
            resource_path_validator: Some(Arc::new(|_| {
                anyhow::bail!("application_owned_path_refused")
            })),
            ..Default::default()
        };
        let error = guard
            .read_resource_path(std::path::Path::new("/resource-denied-before-io"))
            .unwrap_err();
        assert_eq!(error.to_string(), "application_owned_path_refused");
        assert_eq!(guard.observation().refused_operations, 1);
    }

    #[test]
    fn owned_raster_decode_preserves_pixels_and_bounds_animation_work() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            1,
            1,
            image::Rgba([10, 20, 30, 255]),
        ))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
        let frames = decode_owned_image(png.get_ref(), image::ImageFormat::Png).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].buffer().get_pixel(0, 0).0, [30, 20, 10, 255]);
        assert!(validate_owned_image_size(2049, 2049).is_err());
        let mut encoded = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut encoded);
            for _ in 0..=OWNED_HIDDEN_MAX_IMAGE_FRAMES {
                encoder
                    .encode_frame(image::Frame::new(image::RgbaImage::from_pixel(
                        1,
                        1,
                        image::Rgba([10, 20, 30, 255]),
                    )))
                    .unwrap();
            }
        }
        assert!(
            decode_owned_image(&encoded, image::ImageFormat::Gif)
                .unwrap_err()
                .to_string()
                .contains("owned_image_frame_limit")
        );
    }

    #[test]
    fn svgz_decompression_stops_at_the_owned_byte_budget() {
        use std::io::Read as _;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::copy(
            &mut std::io::repeat(0).take(OWNED_HIDDEN_MAX_RESOURCE_BYTES + 1),
            &mut encoder,
        )
        .unwrap();
        let compressed = encoder.finish().unwrap();
        assert!(
            owned_svg_bytes(&compressed)
                .unwrap_err()
                .to_string()
                .contains("owned_svg_decompression_limit")
        );
    }
    #[test]
    fn final_native_gate_rejects_visibility_sheets_tabs_and_interaction() {
        let mut params = WindowParams {
            bounds: Bounds::new(point(px(0.0), px(0.0)), size(px(500.0), px(400.0))),
            titlebar: None,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            focus: false,
            show: false,
            display_id: None,
            window_min_size: None,
            #[cfg(target_os = "macos")]
            tabbing_identifier: None,
        };
        assert!(validate_owned_hidden_params(&params).is_ok());
        params.show = true;
        assert!(validate_owned_hidden_params(&params).is_err());
        params.show = false;
        params.focus = true;
        assert!(validate_owned_hidden_params(&params).is_err());
        params.focus = false;
        params.kind = WindowKind::Dialog;
        assert!(validate_owned_hidden_params(&params).is_err());
        params.kind = WindowKind::PopUp;
        params.is_minimizable = true;
        assert!(validate_owned_hidden_params(&params).is_err());
        params.is_minimizable = false;
        #[cfg(target_os = "macos")]
        {
            params.tabbing_identifier = Some("unsafe".into());
            assert!(validate_owned_hidden_params(&params).is_err());
        }
    }
    use crate::{point, px, size};

    #[test]
    fn allocation_bounds_reject_nonfinite_zero_and_oversized_images() {
        let bounds = |w, h| Bounds::new(point(px(0.0), px(0.0)), size(px(w), px(h)));
        assert!(validate_owned_hidden_bounds(bounds(1024.0, 1024.0), 2.0).is_ok());
        for (w, h) in [(0.0, 20.0), (f32::NAN, 10.0), (2049.0, 2049.0)] {
            assert!(validate_owned_hidden_bounds(bounds(w, h), 1.0).is_err());
        }
        assert!(validate_owned_hidden_bounds(bounds(1025.0, 1024.0), 2.0).is_err());
    }

    #[test]
    fn reservations_and_refusals_are_observed_without_native_effects() {
        let guard = OwnedHiddenGuard::default();
        for _ in 0..OWNED_HIDDEN_MAX_WINDOWS {
            guard.window_opened().unwrap();
        }
        assert!(guard.window_opened().is_err());
        guard.window_closed();
        guard.window_opened().unwrap();
        let observation = guard.observation();
        assert_eq!(observation.live_windows, OWNED_HIDDEN_MAX_WINDOWS);
        assert_eq!(observation.refused_operations, 1);
        assert_eq!(observation.completed_frames, 0);
    }
}

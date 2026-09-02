use crate::{
    AssetSource, DevicePixels, IsZero, RenderImage, Result, SharedString, Size,
    swap_rgba_pa_to_bgra,
};
use image::Frame;
use resvg::tiny_skia::Pixmap;
use smallvec::SmallVec;
use std::{
    hash::Hash,
    sync::{Arc, LazyLock},
};

#[cfg(target_os = "macos")]
const EMOJI_FONT_FAMILIES: &[&str] = &["Apple Color Emoji", ".AppleColorEmojiUI"];

#[cfg(target_os = "windows")]
const EMOJI_FONT_FAMILIES: &[&str] = &["Segoe UI Emoji", "Segoe UI Symbol"];

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
const EMOJI_FONT_FAMILIES: &[&str] = &[
    "Noto Color Emoji",
    "Emoji One",
    "Twitter Color Emoji",
    "JoyPixels",
];

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    target_os = "linux",
    target_os = "freebsd",
)))]
const EMOJI_FONT_FAMILIES: &[&str] = &[];

fn is_emoji_presentation(c: char) -> bool {
    static EMOJI_PRESENTATION_REGEX: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new("\\p{Emoji_Presentation}").unwrap());
    let mut buf = [0u8; 4];
    EMOJI_PRESENTATION_REGEX.is_match(c.encode_utf8(&mut buf))
}

fn font_has_char(db: &usvg::fontdb::Database, id: usvg::fontdb::ID, ch: char) -> bool {
    db.with_face_data(id, |font_data, face_index| {
        ttf_parser::Face::parse(font_data, face_index)
            .ok()
            .and_then(|face| face.glyph_index(ch))
            .is_some()
    })
    .unwrap_or(false)
}

fn select_emoji_font(
    ch: char,
    fonts: &[usvg::fontdb::ID],
    db: &usvg::fontdb::Database,
    families: &[&str],
) -> Option<usvg::fontdb::ID> {
    for family_name in families {
        let query = usvg::fontdb::Query {
            families: &[usvg::fontdb::Family::Name(family_name)],
            weight: usvg::fontdb::Weight(400),
            stretch: usvg::fontdb::Stretch::Normal,
            style: usvg::fontdb::Style::Normal,
        };

        let Some(id) = db.query(&query) else {
            continue;
        };

        if fonts.contains(&id) || !font_has_char(db, id, ch) {
            continue;
        }

        return Some(id);
    }

    None
}

/// When rendering SVGs, we render them at twice the size to get a higher-quality result.
pub const SMOOTH_SVG_SCALE_FACTOR: f32 = 2.;

#[derive(Clone, PartialEq, Hash, Eq)]
#[expect(missing_docs)]
pub struct RenderSvgParams {
    pub path: SharedString,
    pub size: Size<DevicePixels>,
}

#[derive(Clone)]
/// A struct holding everything necessary to render SVGs.
pub struct SvgRenderer {
    asset_source: Arc<dyn AssetSource>,
    usvg_options: Arc<usvg::Options<'static>>,
}

/// The size in which to render the SVG.
pub enum SvgSize {
    /// An absolute size in device pixels.
    Size(Size<DevicePixels>),
    /// A scaling factor to apply to the size provided by the SVG.
    ScaleFactor(f32),
}

impl SvgRenderer {
    /// Creates a new SVG renderer with the provided asset source.
    pub fn new(asset_source: Arc<dyn AssetSource>) -> Self {
        static FONT_DB: LazyLock<Arc<usvg::fontdb::Database>> = LazyLock::new(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        });
        let default_font_resolver = usvg::FontResolver::default_font_selector();
        let font_resolver = Box::new(
            move |font: &usvg::Font, db: &mut Arc<usvg::fontdb::Database>| {
                if db.is_empty() {
                    *db = FONT_DB.clone();
                }
                default_font_resolver(font, db)
            },
        );
        let default_fallback_selection = usvg::FontResolver::default_fallback_selector();
        let fallback_selection = Box::new(
            move |ch: char, fonts: &[usvg::fontdb::ID], db: &mut Arc<usvg::fontdb::Database>| {
                if is_emoji_presentation(ch) {
                    if let Some(id) = select_emoji_font(ch, fonts, db.as_ref(), EMOJI_FONT_FAMILIES)
                    {
                        return Some(id);
                    }
                }

                default_fallback_selection(ch, fonts, db)
            },
        );
        let options = usvg::Options {
            font_resolver: usvg::FontResolver {
                select_font: font_resolver,
                select_fallback: fallback_selection,
            },
            ..Default::default()
        };
        Self {
            asset_source,
            usvg_options: Arc::new(options),
        }
    }

    /// Renders the given bytes into an image buffer.
    pub fn render_single_frame(
        &self,
        bytes: &[u8],
        scale_factor: f32,
        to_brga: bool,
    ) -> Result<Arc<RenderImage>> {
        self.render_pixmap(
            bytes,
            SvgSize::ScaleFactor(scale_factor * SMOOTH_SVG_SCALE_FACTOR),
        )
        .map(|pixmap| {
            let mut buffer =
                image::ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take())
                    .unwrap();

            if to_brga {
                for pixel in buffer.chunks_exact_mut(4) {
                    swap_rgba_pa_to_bgra(pixel);
                }
            }

            let mut image = RenderImage::new(SmallVec::from_const([Frame::new(buffer)]));
            image.scale_factor = SMOOTH_SVG_SCALE_FACTOR;
            Arc::new(image)
        })
    }

    pub(crate) fn render_alpha_mask(
        &self,
        params: &RenderSvgParams,
        bytes: Option<&[u8]>,
    ) -> Result<Option<(Size<DevicePixels>, Vec<u8>)>> {
        anyhow::ensure!(!params.size.is_zero(), "can't render at a zero size");

        let render_pixmap = |bytes| {
            let pixmap = self.render_pixmap(bytes, SvgSize::Size(params.size))?;

            // Convert the pixmap's pixels into an alpha mask.
            let size = Size::new(
                DevicePixels(pixmap.width() as i32),
                DevicePixels(pixmap.height() as i32),
            );
            let alpha_mask = pixmap
                .pixels()
                .iter()
                .map(|p| p.alpha())
                .collect::<Vec<_>>();

            Ok(Some((size, alpha_mask)))
        };

        if let Some(bytes) = bytes {
            render_pixmap(bytes)
        } else if let Some(bytes) = self.asset_source.load(&params.path)? {
            render_pixmap(&bytes)
        } else {
            Ok(None)
        }
    }

    fn owned_tree(&self, bytes: &[u8], guard: &crate::OwnedHiddenGuard) -> Result<usvg::Tree> {
        use std::sync::atomic::{AtomicU64, Ordering};
        let bytes = crate::owned_svg_bytes(bytes)?;
        let failure = parking_lot::Mutex::new(None::<anyhow::Error>);
        let resource_bytes = AtomicU64::new(bytes.len() as u64);
        let resource_pixels = AtomicU64::new(0);
        let default_data = usvg::ImageHrefResolver::default_data_resolver();
        let resolve_data = |mime: &str, data: Arc<Vec<u8>>, options: &usvg::Options<'_>| {
            if failure.lock().is_some() {
                return None;
            }
            let result = (|| -> Result<Option<usvg::ImageKind>> {
                let data = if data.starts_with(&[0x1f, 0x8b]) {
                    Arc::new(crate::owned_svg_bytes(data.as_slice())?.into_owned())
                } else {
                    data
                };
                resource_bytes
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                        used.checked_add(data.len() as u64)
                            .filter(|next| *next <= crate::OWNED_HIDDEN_MAX_RESOURCE_BYTES)
                    })
                    .map_err(|_| guard.refuse("svg_resource_byte_limit"))?;
                if let Ok(format) = image::guess_format(&data) {
                    let mut reader = image::ImageReader::with_format(
                        std::io::Cursor::new(data.as_slice()),
                        format,
                    );
                    reader.limits(crate::owned_image_limits());
                    let (width, height) = reader.into_dimensions()?;
                    let pixels = crate::validate_owned_image_size(width, height)?;
                    resource_pixels
                        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                            used.checked_add(pixels)
                                .filter(|next| *next <= crate::OWNED_HIDDEN_MAX_PIXELS)
                        })
                        .map_err(|_| guard.refuse("svg_resource_pixel_limit"))?;
                }
                Ok(default_data(mime, data, options))
            })();
            match result {
                Ok(image) => image,
                Err(error) => {
                    let mut failure = failure.lock();
                    if failure.is_none() {
                        *failure = Some(error);
                    }
                    None
                }
            }
        };
        let options = usvg::Options {
            fontdb: self.usvg_options.fontdb.clone(),
            font_resolver: usvg::FontResolver {
                select_font: Box::new(|font, db| {
                    (self.usvg_options.font_resolver.select_font)(font, db)
                }),
                select_fallback: Box::new(|ch, fonts, db| {
                    (self.usvg_options.font_resolver.select_fallback)(ch, fonts, db)
                }),
            },
            image_href_resolver: usvg::ImageHrefResolver {
                resolve_data: Box::new(|mime, data, options| resolve_data(mime, data, options)),
                resolve_string: Box::new(|href, options| {
                    if failure.lock().is_some() {
                        return None;
                    }
                    let result = if href.contains("://") {
                        Err(guard.refuse("network_resource"))
                    } else {
                        guard.read_resource_path(&options.get_abs_path(std::path::Path::new(href)))
                    };
                    match result {
                        Ok(data) => resolve_data("text/plain", Arc::new(data), options),
                        Err(error) => {
                            let mut failure = failure.lock();
                            if failure.is_none() {
                                *failure = Some(error);
                            }
                            None
                        }
                    }
                }),
            },
            ..Default::default()
        };
        let tree = usvg::Tree::from_str(std::str::from_utf8(&bytes)?, &options);
        if let Some(error) = failure.lock().take() {
            return Err(error);
        }
        Ok(tree?)
    }

    fn render_pixmap(&self, bytes: &[u8], size: SvgSize) -> Result<Pixmap> {
        let tree = if let Some(guard) = crate::OwnedHiddenGuard::installed() {
            self.owned_tree(bytes, guard)?
        } else {
            usvg::Tree::from_data(bytes, &self.usvg_options)?
        };
        let svg_size = tree.size();
        let scale = match size {
            SvgSize::Size(size) => size.width.0 as f32 / svg_size.width(),
            SvgSize::ScaleFactor(scale) => scale,
        };
        if crate::OwnedHiddenGuard::installed().is_some() {
            let width = svg_size.width() * scale;
            let height = svg_size.height() * scale;
            anyhow::ensure!(
                width.is_finite()
                    && height.is_finite()
                    && width >= 1.0
                    && height >= 1.0
                    && width <= crate::OWNED_HIDDEN_MAX_PIXELS as f32
                    && height <= crate::OWNED_HIDDEN_MAX_PIXELS as f32,
                "owned_svg_pixel_limit"
            );
            crate::validate_owned_image_size(width as u32, height as u32)?;
        }

        // Render the SVG to a pixmap with the specified width and height.
        let mut pixmap = resvg::tiny_skia::Pixmap::new(
            (svg_size.width() * scale) as u32,
            (svg_size.height() * scale) as u32,
        )
        .ok_or(usvg::Error::InvalidSize)?;

        let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);

        resvg::render(&tree, transform, &mut pixmap.as_mut());

        Ok(pixmap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_svg_refusal_fails_the_whole_load_instead_of_omitting_the_image() {
        let renderer = SvgRenderer::new(Arc::new(()));
        let guard = crate::OwnedHiddenGuard::default();
        let local = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><image href="/unapproved-image.png" width="10" height="10"/><image href="https://example.invalid/must-not-run.png" width="10" height="10"/></svg>"#;
        let error = renderer.owned_tree(local, &guard).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("resource_path_validator_missing")
        );
        let network = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><image href="https://example.invalid/image.png" width="10" height="10"/></svg>"#;
        assert!(
            renderer
                .owned_tree(network, &guard)
                .unwrap_err()
                .to_string()
                .contains("network_resource")
        );
        let embedded = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>"#;
        assert!(renderer.owned_tree(embedded, &guard).is_ok());
        assert_eq!(guard.observation().refused_operations, 2);
    }

    const IBM_PLEX_REGULAR: &[u8] =
        include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf");
    const LILEX_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/lilex/Lilex-Regular.ttf");

    #[test]
    fn test_is_emoji_presentation() {
        let cases = [
            ("a", false),
            ("Z", false),
            ("1", false),
            ("#", false),
            ("*", false),
            ("漢", false),
            ("中", false),
            ("カ", false),
            ("©", false),
            ("♥", false),
            ("😀", true),
            ("✅", true),
            ("🇺🇸", true),
            // SVG fallback is not cluster-aware yet
            ("©️", false),
            ("♥️", false),
            ("1️⃣", false),
        ];
        for (s, expected) in cases {
            assert_eq!(
                is_emoji_presentation(s.chars().next().unwrap()),
                expected,
                "for char {:?}",
                s
            );
        }
    }

    #[test]
    fn test_select_emoji_font_skips_family_without_glyph() {
        let mut db = usvg::fontdb::Database::new();

        db.load_font_data(IBM_PLEX_REGULAR.to_vec());
        db.load_font_data(LILEX_REGULAR.to_vec());

        let ibm_plex_sans = db
            .query(&usvg::fontdb::Query {
                families: &[usvg::fontdb::Family::Name("IBM Plex Sans")],
                weight: usvg::fontdb::Weight(400),
                stretch: usvg::fontdb::Stretch::Normal,
                style: usvg::fontdb::Style::Normal,
            })
            .unwrap();
        let lilex = db
            .query(&usvg::fontdb::Query {
                families: &[usvg::fontdb::Family::Name("Lilex")],
                weight: usvg::fontdb::Weight(400),
                stretch: usvg::fontdb::Stretch::Normal,
                style: usvg::fontdb::Style::Normal,
            })
            .unwrap();
        let selected = select_emoji_font('│', &[], &db, &["IBM Plex Sans", "Lilex"]).unwrap();

        assert_eq!(selected, lilex);
        assert!(!font_has_char(&db, ibm_plex_sans, '│'));
        assert!(font_has_char(&db, selected, '│'));
    }
}

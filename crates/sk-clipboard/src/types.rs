/// Content types for clipboard entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum ContentType {
    Text,
    Image,
    Link,
    File,
    Color,
}

impl ContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentType::Text => "text",
            ContentType::Image => "image",
            ContentType::Link => "link",
            ContentType::File => "file",
            ContentType::Color => "color",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        <Self as std::str::FromStr>::from_str(s).unwrap_or(Self::Text)
    }
}

#[allow(dead_code)]
pub fn classify_content(text: &str, has_image: bool) -> ContentType {
    if has_image {
        return ContentType::Image;
    }

    if text.starts_with("http://") || text.starts_with("https://") || text.contains("://") {
        return ContentType::Link;
    }

    if text.starts_with('/') || text.starts_with('~') {
        return ContentType::File;
    }

    ContentType::Text
}

/// A single clipboard history entry (full, includes content).
#[derive(Debug, Clone)]
pub struct ClipboardEntry {
    pub id: String,
    pub content: String,
    pub content_type: ContentType,
    pub timestamp: i64,
    pub pinned: bool,
    /// OCR text extracted from images (None for text entries or pending OCR).
    #[allow(dead_code)]
    pub ocr_text: Option<String>,
    /// Human-readable source application name (for example, "Safari").
    #[allow(dead_code)]
    pub source_app_name: Option<String>,
    /// Source application bundle identifier (for example, "com.apple.Safari").
    #[allow(dead_code)]
    pub source_app_bundle_id: Option<String>,
}

/// Lightweight clipboard entry metadata for list views (no payload).
///
/// This struct contains everything needed for displaying entries in a list
/// without loading the full content (which can be megabytes for images).
/// Use the app-owned content service to fetch the full content when needed.
#[derive(Debug, Clone)]
pub struct ClipboardEntryMeta {
    pub id: String,
    pub content_type: ContentType,
    pub timestamp: i64,
    pub pinned: bool,
    /// First 100 chars of text content (for list preview), or "[Image]" for images.
    pub text_preview: String,
    /// Image width in pixels (None for text).
    pub image_width: Option<u32>,
    /// Image height in pixels (None for text).
    pub image_height: Option<u32>,
    /// Content size in bytes (useful for displaying file sizes).
    #[allow(dead_code)]
    pub byte_size: usize,
    /// OCR text extracted from images (None for text entries or pending OCR).
    #[allow(dead_code)]
    pub ocr_text: Option<String>,
}

impl ClipboardEntryMeta {
    /// Get a display-friendly preview string for list items.
    pub fn display_preview(&self) -> String {
        match self.content_type {
            ContentType::Image => {
                if let (Some(w), Some(h)) = (self.image_width, self.image_height) {
                    format!("{}×{} image", w, h)
                } else {
                    "[Image]".to_string()
                }
            }
            ContentType::Text | ContentType::Link | ContentType::File | ContentType::Color => {
                let sanitized = self.text_preview.replace(['\n', '\r'], " ");
                // Gate the ellipsis on the *character* count, not the byte length:
                // `chars().take(50)` truncates by characters, so using `len()` (bytes)
                // here appended a "…" to short multibyte text (emoji/CJK) that was
                // never actually truncated.
                if sanitized.chars().count() > 50 {
                    format!("{}…", sanitized.chars().take(50).collect::<String>())
                } else {
                    sanitized
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_conversion_is_stable() {
        assert_eq!(ContentType::Text.as_str(), "text");
        assert_eq!(ContentType::Image.as_str(), "image");
        assert_eq!(ContentType::Link.as_str(), "link");
        assert_eq!(ContentType::File.as_str(), "file");
        assert_eq!(ContentType::Color.as_str(), "color");
        assert_eq!(ContentType::from_str("text"), ContentType::Text);
        assert_eq!(ContentType::from_str("image"), ContentType::Image);
        assert_eq!(ContentType::from_str("link"), ContentType::Link);
        assert_eq!(ContentType::from_str("file"), ContentType::File);
        assert_eq!(ContentType::from_str("color"), ContentType::Color);
        assert_eq!(ContentType::from_str("unknown"), ContentType::Text);
    }

    #[test]
    fn classify_content_prefers_image_payloads() {
        assert_eq!(
            classify_content("https://example.com", true),
            ContentType::Image
        );
    }

    #[test]
    fn classify_content_recognizes_links() {
        assert_eq!(
            classify_content("http://example.com", false),
            ContentType::Link
        );
        assert_eq!(
            classify_content("https://example.com", false),
            ContentType::Link
        );
        assert_eq!(
            classify_content("custom-scheme://resource", false),
            ContentType::Link
        );
    }

    #[test]
    fn classify_content_recognizes_paths() {
        assert_eq!(classify_content("/tmp/file.txt", false), ContentType::File);
        assert_eq!(classify_content("~/notes.md", false), ContentType::File);
    }

    #[test]
    fn classify_content_defaults_to_text() {
        assert_eq!(
            classify_content("just some plain text", false),
            ContentType::Text
        );
    }

    fn meta(content_type: ContentType, preview: &str) -> ClipboardEntryMeta {
        ClipboardEntryMeta {
            id: "clip-1".to_string(),
            content_type,
            timestamp: 1_778_000_000_000,
            pinned: false,
            text_preview: preview.to_string(),
            image_width: None,
            image_height: None,
            byte_size: preview.len(),
            ocr_text: None,
        }
    }

    #[test]
    fn display_preview_preserves_image_dimensions_and_fallback() {
        let mut entry = meta(ContentType::Image, "[Image]");
        assert_eq!(entry.display_preview(), "[Image]");
        entry.image_width = Some(1440);
        entry.image_height = Some(900);
        assert_eq!(entry.display_preview(), "1440×900 image");
    }

    #[test]
    fn display_preview_does_not_add_ellipsis_for_short_multibyte_text() {
        // Regression: the truncation threshold used `sanitized.len()` (bytes) while
        // the truncation itself used `chars().take(50)`. A short-but-multibyte
        // preview (e.g. 30 two-byte chars = 60 bytes) tripped the byte threshold and
        // got a "…" appended even though nothing was actually cut.
        let text = "é".repeat(30); // 30 chars, 60 bytes
        let entry = meta(ContentType::Text, &text);
        let preview = entry.display_preview();
        assert!(
            !preview.ends_with('…'),
            "no ellipsis when nothing is truncated, got: {preview:?}"
        );
        assert_eq!(preview.chars().count(), 30);
    }

    #[test]
    fn display_preview_truncates_long_multibyte_text_to_50_chars() {
        // 60 multibyte chars > 50-char limit → truncate to exactly 50 chars + "…".
        let text = "🌊".repeat(60);
        let entry = meta(ContentType::Text, &text);
        let preview = entry.display_preview();
        assert!(preview.ends_with('…'));
        assert_eq!(preview.chars().count(), 51); // 50 content chars + ellipsis
    }

    #[test]
    fn display_preview_flattens_and_truncates_text() {
        let entry = meta(
            ContentType::Text,
            "first line\r\nsecond line with enough trailing text to exceed fifty characters",
        );
        let preview = entry.display_preview();
        assert!(!preview.contains(['\r', '\n']));
        assert!(preview.ends_with('…'));
        assert_eq!(preview.chars().count(), 51);
    }
}

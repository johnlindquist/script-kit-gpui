use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LauncherContextKind {
    Url,
    FilePath,
    DirectoryPath,
    Image,
    Code,
    PlainText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LauncherContextSource {
    Clipboard,
    SelectedText,
    BrowserUrl,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherContextItem {
    pub source: LauncherContextSource,
    pub kind: LauncherContextKind,
    pub label: String,
    pub preview: String,
    #[serde(skip)]
    pub captured_at: Instant,
    pub ttl_secs: u64,
}

impl LauncherContextItem {
    pub fn is_fresh(&self) -> bool {
        self.captured_at.elapsed().as_secs() < self.ttl_secs
    }

    pub fn chip_label(&self) -> String {
        let source = match self.source {
            LauncherContextSource::Clipboard => "Clipboard",
            LauncherContextSource::SelectedText => "Selected",
            LauncherContextSource::BrowserUrl => "Browser",
        };
        let kind = match self.kind {
            LauncherContextKind::Url => "URL",
            LauncherContextKind::FilePath => "File",
            LauncherContextKind::DirectoryPath => "Folder",
            LauncherContextKind::Image => "Image",
            LauncherContextKind::Code => "Code",
            LauncherContextKind::PlainText => "Text",
        };
        format!("{source}: {kind}")
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherContextSnapshot {
    pub generation: u64,
    pub items: Vec<LauncherContextItem>,
}

impl LauncherContextSnapshot {
    pub fn fresh_items(&self) -> impl Iterator<Item = &LauncherContextItem> {
        self.items.iter().filter(|item| item.is_fresh())
    }

    pub fn has_kind(&self, kind: LauncherContextKind) -> bool {
        self.fresh_items().any(|item| item.kind == kind)
    }

    pub fn primary_kind(&self) -> Option<LauncherContextKind> {
        self.fresh_items().next().map(|item| item.kind)
    }

    pub fn cache_key_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.generation.hash(&mut hasher);
        for item in &self.items {
            item.source.hash(&mut hasher);
            item.kind.hash(&mut hasher);
        }
        hasher.finish()
    }
}

const CLIPBOARD_TTL_SECS: u64 = 300;

pub fn detect_content_kind(text: &str) -> LauncherContextKind {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return LauncherContextKind::PlainText;
    }
    if looks_like_url(trimmed) {
        return LauncherContextKind::Url;
    }
    if looks_like_directory(trimmed) {
        return LauncherContextKind::DirectoryPath;
    }
    if looks_like_file_path(trimmed) {
        return LauncherContextKind::FilePath;
    }
    if looks_like_code(trimmed) {
        return LauncherContextKind::Code;
    }
    LauncherContextKind::PlainText
}

fn looks_like_url(s: &str) -> bool {
    s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("file://")
        || s.starts_with("mailto:")
}

fn looks_like_file_path(s: &str) -> bool {
    (s.starts_with('/') || s.starts_with('~')) && !s.contains('\n') && s.len() < 512
}

fn looks_like_directory(s: &str) -> bool {
    looks_like_file_path(s) && s.ends_with('/')
}

fn looks_like_code(s: &str) -> bool {
    let indicators = [
        "fn ",
        "pub ",
        "let ",
        "const ",
        "import ",
        "export ",
        "function ",
        "class ",
        "def ",
        "return ",
        "if (",
        "for (",
        "while (",
        "=> {",
        "-> {",
        "};",
        "});",
    ];
    let line_count = s.lines().count();
    if line_count < 2 {
        return false;
    }
    indicators.iter().any(|ind| s.contains(ind))
}

#[cfg(target_os = "macos")]
fn read_clipboard_text() -> Option<String> {
    use cocoa::appkit::NSPasteboard;
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let pasteboard: id = NSPasteboard::generalPasteboard(nil);
        if pasteboard == nil {
            return None;
        }
        let ns_string_type = NSString::alloc(nil).init_str("public.utf8-plain-text");
        let string: id = msg_send![pasteboard, stringForType: ns_string_type];
        if string == nil {
            return None;
        }
        let bytes = string.UTF8String() as *const u8;
        if bytes.is_null() {
            return None;
        }
        let len = string.len();
        let slice = std::slice::from_raw_parts(bytes, len);
        String::from_utf8(slice.to_vec()).ok()
    }
}

#[cfg(not(target_os = "macos"))]
fn read_clipboard_text() -> Option<String> {
    None
}

pub fn capture_clipboard_context() -> Option<LauncherContextItem> {
    let text = read_clipboard_text()?;
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > 8192 {
        return None;
    }
    if looks_like_sensitive(trimmed) {
        return None;
    }
    let kind = detect_content_kind(trimmed);
    let preview = cap_preview(trimmed, 160);
    Some(LauncherContextItem {
        source: LauncherContextSource::Clipboard,
        kind,
        label: format!("Clipboard: {}", kind_label(kind)),
        preview,
        captured_at: Instant::now(),
        ttl_secs: CLIPBOARD_TTL_SECS,
    })
}

pub fn capture_launcher_context(generation: u64) -> LauncherContextSnapshot {
    let mut items = Vec::new();
    if let Some(clip) = capture_clipboard_context() {
        items.push(clip);
    }
    LauncherContextSnapshot { generation, items }
}

fn kind_label(kind: LauncherContextKind) -> &'static str {
    match kind {
        LauncherContextKind::Url => "URL",
        LauncherContextKind::FilePath => "File",
        LauncherContextKind::DirectoryPath => "Folder",
        LauncherContextKind::Image => "Image",
        LauncherContextKind::Code => "Code",
        LauncherContextKind::PlainText => "Text",
    }
}

fn looks_like_sensitive(s: &str) -> bool {
    let patterns = [
        "-----BEGIN",
        "sk-",
        "ghp_",
        "github_pat_",
        "AKIA",
        "password=",
        "token=",
        "secret=",
        "api_key=",
        "Bearer ",
    ];
    patterns.iter().any(|p| s.contains(p))
}

/// Truncate `s` to at most `max` bytes for a preview, appending `…` when cut.
///
/// Floors the cut to a UTF-8 char boundary: a naive `&s[..max]` panicked when a
/// multibyte char straddled byte `max` (the value is external clipboard text).
pub(crate) fn cap_preview(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max.min(s.len());
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

pub fn context_boost_for_result(
    result: &crate::scripts::SearchResult,
    context: &LauncherContextSnapshot,
) -> i32 {
    use crate::scripts::SearchResult;

    let fresh: Vec<&LauncherContextItem> = context.fresh_items().collect();
    if fresh.is_empty() {
        return 0;
    }

    let has_url = fresh.iter().any(|i| i.kind == LauncherContextKind::Url);
    let has_file = fresh.iter().any(|i| {
        matches!(
            i.kind,
            LauncherContextKind::FilePath | LauncherContextKind::DirectoryPath
        )
    });
    let has_image = fresh.iter().any(|i| i.kind == LauncherContextKind::Image);
    let has_code = fresh.iter().any(|i| i.kind == LauncherContextKind::Code);

    let name_lower = result.name().to_lowercase();
    let desc_lower = result.description().unwrap_or("").to_lowercase();

    let keywords = match result {
        SearchResult::BuiltIn(bm) => bm
            .entry
            .keywords
            .iter()
            .map(|k| k.to_lowercase())
            .collect::<Vec<_>>(),
        _ => vec![],
    };

    let mut boost = 0i32;

    if has_url
        && (name_lower.contains("url")
            || name_lower.contains("link")
            || name_lower.contains("browser")
            || name_lower.contains("summarize")
            || desc_lower.contains("url")
            || keywords
                .iter()
                .any(|k| k.contains("url") || k.contains("link")))
    {
        boost = boost.max(200);
    }

    if has_file
        && (name_lower.contains("file")
            || name_lower.contains("finder")
            || name_lower.contains("open")
            || name_lower.contains("reveal")
            || desc_lower.contains("file")
            || keywords
                .iter()
                .any(|k| k.contains("file") || k.contains("path")))
    {
        boost = boost.max(180);
    }

    if has_image
        && (name_lower.contains("image")
            || name_lower.contains("screenshot")
            || name_lower.contains("photo")
            || name_lower.contains("ocr")
            || desc_lower.contains("image")
            || keywords.iter().any(|k| k.contains("image")))
    {
        boost = boost.max(180);
    }

    if has_code
        && (name_lower.contains("code")
            || name_lower.contains("snippet")
            || name_lower.contains("format")
            || desc_lower.contains("code")
            || keywords.iter().any(|k| k.contains("code")))
    {
        boost = boost.max(150);
    }

    boost.min(450)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_preview_survives_multibyte_at_cut_boundary() {
        // Regression: `&s[..max]` byte-sliced under a byte-length guard; clipboard
        // text with a multibyte char straddling byte `max` panicked.
        let s = format!("{}{}", "a".repeat(159), "é"); // byte 160 is inside 'é'
        let out = cap_preview(&s, 160);
        assert!(out.ends_with('…'));
        assert_eq!(&out[..159], "a".repeat(159));
        // 'é' at the boundary is dropped rather than split; no panic, valid UTF-8.
        assert!(!out.contains('\u{fffd}'));

        // Emoji straddling the cut.
        let s = format!("{}{}", "x".repeat(158), "🌊🌊");
        let out = cap_preview(&s, 160);
        assert!(out.ends_with('…'));
        assert!(out.is_char_boundary(out.len()));

        // Short input is returned unchanged, no ellipsis.
        assert_eq!(cap_preview("héllo", 160), "héllo");
    }

    #[test]
    fn detect_url() {
        assert_eq!(
            detect_content_kind("https://example.com/page"),
            LauncherContextKind::Url
        );
        assert_eq!(
            detect_content_kind("http://localhost:3000"),
            LauncherContextKind::Url
        );
    }

    #[test]
    fn detect_file_path() {
        assert_eq!(
            detect_content_kind("/Users/john/file.txt"),
            LauncherContextKind::FilePath
        );
        assert_eq!(
            detect_content_kind("~/Documents/"),
            LauncherContextKind::DirectoryPath
        );
    }

    #[test]
    fn detect_code() {
        let code = "fn main() {\n    println!(\"hello\");\n}";
        assert_eq!(detect_content_kind(code), LauncherContextKind::Code);
    }

    #[test]
    fn detect_plain_text() {
        assert_eq!(
            detect_content_kind("Hello world"),
            LauncherContextKind::PlainText
        );
    }

    #[test]
    fn sensitive_detection() {
        assert!(looks_like_sensitive("sk-proj-abc123"));
        assert!(looks_like_sensitive("ghp_xxxxxxxxxxxx"));
        assert!(looks_like_sensitive("password=secret"));
        assert!(!looks_like_sensitive("hello world"));
    }

    #[test]
    fn chip_label_format() {
        let item = LauncherContextItem {
            source: LauncherContextSource::Clipboard,
            kind: LauncherContextKind::Url,
            label: "Clipboard: URL".to_string(),
            preview: "https://example.com".to_string(),
            captured_at: Instant::now(),
            ttl_secs: 300,
        };
        assert_eq!(item.chip_label(), "Clipboard: URL");
    }

    #[test]
    fn freshness_check() {
        let fresh = LauncherContextItem {
            source: LauncherContextSource::Clipboard,
            kind: LauncherContextKind::Url,
            label: "test".to_string(),
            preview: "test".to_string(),
            captured_at: Instant::now(),
            ttl_secs: 300,
        };
        assert!(fresh.is_fresh());
    }

    #[test]
    fn snapshot_cache_key() {
        let s1 = LauncherContextSnapshot {
            generation: 1,
            items: vec![],
        };
        let s2 = LauncherContextSnapshot {
            generation: 2,
            items: vec![],
        };
        assert_ne!(s1.cache_key_hash(), s2.cache_key_hash());
    }

    #[test]
    fn preview_capping() {
        let short = cap_preview("hello", 160);
        assert_eq!(short, "hello");

        let long = cap_preview(&"x".repeat(200), 160);
        assert!(long.len() < 200);
        assert!(long.ends_with('…'));
    }
}

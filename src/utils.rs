//! Shared utility functions for Script Kit GPUI

/// Strip HTML tags from a string, returning plain text.
///
/// This function removes all HTML tags (content between < and >) and normalizes
/// whitespace. Consecutive whitespace is collapsed to a single space, and the
/// result is trimmed.
///
/// # Examples
///
/// ```
/// use script_kit_gpui::utils::strip_html_tags;
///
/// assert_eq!(strip_html_tags("<p>Hello</p>"), "Hello");
/// assert_eq!(strip_html_tags("<div><span>A</span><span>B</span></div>"), "A B");
/// ```
#[allow(dead_code)]
pub fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut pending_space = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                pending_space = true; // Add space between tags
            }
            _ if !in_tag => {
                if ch.is_whitespace() {
                    if !result.is_empty() && !result.ends_with(' ') {
                        pending_space = true;
                    }
                } else {
                    if pending_space && !result.is_empty() {
                        result.push(' ');
                    }
                    pending_space = false;
                    result.push(ch);
                }
            }
            _ => {} // Skip characters inside tags
        }
    }

    result.trim().to_string()
}

// ============================================================================
// HTML Parsing and Element Types
// ============================================================================

/// Represents a parsed HTML element with its type and content
#[derive(Debug, Clone, PartialEq)]
pub enum HtmlElement {
    /// Plain text content
    Text(String),
    /// Header (h1-h6) with level (1-6) and content
    Header {
        level: u8,
        children: Vec<HtmlElement>,
    },
    /// Paragraph
    Paragraph(Vec<HtmlElement>),
    /// Bold/Strong text
    Bold(Vec<HtmlElement>),
    /// Italic/Emphasis text
    Italic(Vec<HtmlElement>),
    /// Inline code with monospace styling
    InlineCode(String),
    /// Code block with optional language
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    /// Unordered list
    UnorderedList(Vec<HtmlElement>),
    /// Ordered list
    OrderedList(Vec<HtmlElement>),
    /// List item
    ListItem(Vec<HtmlElement>),
    /// Blockquote
    Blockquote(Vec<HtmlElement>),
    /// Horizontal rule
    HorizontalRule,
    /// Link with href and text
    Link {
        href: String,
        children: Vec<HtmlElement>,
    },
    /// Line break
    LineBreak,
    /// Div container with optional class attribute
    Div {
        classes: Option<String>,
        children: Vec<HtmlElement>,
    },
    /// Span inline container with optional class attribute
    Span {
        classes: Option<String>,
        children: Vec<HtmlElement>,
    },
}

/// Parser state for HTML parsing
struct HtmlParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> HtmlParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn remaining(&self) -> &str {
        &self.input[self.pos..]
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek_char(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn advance(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.input.len());
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.advance(1);
            } else {
                break;
            }
        }
    }

    /// Check if the remaining input starts with a string (case-insensitive for tags)
    fn starts_with_ci(&self, s: &str) -> bool {
        self.remaining()
            .get(..s.len())
            .map(|prefix| prefix.eq_ignore_ascii_case(s))
            .unwrap_or(false)
    }

    /// Parse an opening tag, returns (tag_name, attributes) or None
    fn parse_opening_tag(&mut self) -> Option<(String, Vec<(String, String)>)> {
        if !self.remaining().starts_with('<') {
            return None;
        }

        // Check for closing tag, comment, or doctype
        if self.remaining().starts_with("</")
            || self.remaining().starts_with("<!--")
            || self.remaining().starts_with("<!")
        {
            return None;
        }

        self.advance(1); // skip '<'

        // Parse tag name
        let mut tag_name = String::new();
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                tag_name.push(c.to_ascii_lowercase());
                self.advance(1);
            } else {
                break;
            }
        }

        if tag_name.is_empty() {
            return None;
        }

        // Parse attributes
        let mut attributes = Vec::new();
        loop {
            self.skip_whitespace();

            if let Some(c) = self.peek_char() {
                if c == '>' {
                    self.advance(1);
                    break;
                }
                if c == '/' {
                    // Self-closing tag
                    self.advance(1);
                    self.skip_whitespace();
                    if self.peek_char() == Some('>') {
                        self.advance(1);
                    }
                    break;
                }

                // Parse attribute name
                let mut attr_name = String::new();
                while let Some(c) = self.peek_char() {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        attr_name.push(c.to_ascii_lowercase());
                        self.advance(1);
                    } else {
                        break;
                    }
                }

                if attr_name.is_empty() {
                    self.advance(1); // Skip unknown character
                    continue;
                }

                self.skip_whitespace();

                let attr_value = if self.peek_char() == Some('=') {
                    self.advance(1); // skip '='
                    self.skip_whitespace();
                    self.parse_attribute_value()
                } else {
                    String::new()
                };

                attributes.push((attr_name, attr_value));
            } else {
                break;
            }
        }

        Some((tag_name, attributes))
    }

    fn parse_attribute_value(&mut self) -> String {
        let quote_char = self.peek_char();
        if quote_char == Some('"') || quote_char == Some('\'') {
            let quote = quote_char.unwrap();
            self.advance(1); // skip opening quote

            let mut value = String::new();
            while let Some(c) = self.peek_char() {
                if c == quote {
                    self.advance(1);
                    break;
                }
                value.push(c);
                self.advance(1);
            }
            value
        } else {
            // Unquoted value
            let mut value = String::new();
            while let Some(c) = self.peek_char() {
                if c.is_whitespace() || c == '>' || c == '/' {
                    break;
                }
                value.push(c);
                self.advance(1);
            }
            value
        }
    }

    /// Parse a closing tag, returns tag name or None
    fn parse_closing_tag(&mut self, expected: &str) -> bool {
        let close_tag = format!("</{}>", expected);
        if self.starts_with_ci(&close_tag) {
            self.advance(close_tag.len());
            true
        } else {
            false
        }
    }

    /// Parse text content until a tag is encountered
    fn parse_text(&mut self) -> String {
        let mut text = String::new();
        while let Some(c) = self.peek_char() {
            if c == '<' {
                break;
            }
            text.push(c);
            self.advance(1);
        }

        // Decode common HTML entities
        decode_html_entities(&text)
    }

    /// Parse children until a closing tag is found
    fn parse_children(&mut self, end_tag: &str) -> Vec<HtmlElement> {
        let mut children = Vec::new();

        while !self.is_eof() {
            // Check for closing tag
            let close_tag = format!("</{}>", end_tag);
            if self.starts_with_ci(&close_tag) {
                self.advance(close_tag.len());
                break;
            }

            if let Some(element) = self.parse_element() {
                children.push(element);
            } else if !self.is_eof() {
                // If we can't parse an element, try to parse text
                let text = self.parse_text();
                if !text.is_empty() {
                    children.push(HtmlElement::Text(text));
                }
            }
        }

        children
    }

    /// Parse a single element (tag or text)
    fn parse_element(&mut self) -> Option<HtmlElement> {
        if self.is_eof() {
            return None;
        }

        // Check for text first
        if self.peek_char() != Some('<') {
            let text = self.parse_text();
            if !text.is_empty() {
                return Some(HtmlElement::Text(text));
            }
            return None;
        }

        // Check for closing tag (shouldn't happen at this level)
        if self.remaining().starts_with("</") {
            return None;
        }

        // Check for comment
        if self.remaining().starts_with("<!--") {
            // Skip comment
            if let Some(end) = self.remaining().find("-->") {
                self.advance(end + 3);
            }
            return self.parse_element();
        }

        // Parse opening tag
        let (tag_name, attributes) = self.parse_opening_tag()?;

        // Handle self-closing tags
        match tag_name.as_str() {
            "br" => return Some(HtmlElement::LineBreak),
            "hr" => return Some(HtmlElement::HorizontalRule),
            _ => {}
        }

        // Parse children and closing tag
        match tag_name.as_str() {
            "h1" => {
                let children = self.parse_children("h1");
                Some(HtmlElement::Header { level: 1, children })
            }
            "h2" => {
                let children = self.parse_children("h2");
                Some(HtmlElement::Header { level: 2, children })
            }
            "h3" => {
                let children = self.parse_children("h3");
                Some(HtmlElement::Header { level: 3, children })
            }
            "h4" => {
                let children = self.parse_children("h4");
                Some(HtmlElement::Header { level: 4, children })
            }
            "h5" => {
                let children = self.parse_children("h5");
                Some(HtmlElement::Header { level: 5, children })
            }
            "h6" => {
                let children = self.parse_children("h6");
                Some(HtmlElement::Header { level: 6, children })
            }
            "p" => {
                let children = self.parse_children("p");
                Some(HtmlElement::Paragraph(children))
            }
            "strong" | "b" => {
                let children = self.parse_children(&tag_name);
                Some(HtmlElement::Bold(children))
            }
            "em" | "i" => {
                let children = self.parse_children(&tag_name);
                Some(HtmlElement::Italic(children))
            }
            "code" => {
                // Check if parent is a pre (handled at pre level)
                let text = self.parse_text();
                self.parse_closing_tag("code");
                Some(HtmlElement::InlineCode(text))
            }
            "pre" => {
                // Look for code block inside
                let saved_pos = self.pos;
                if let Some((inner_tag, inner_attrs)) = self.parse_opening_tag() {
                    if inner_tag == "code" {
                        let language =
                            inner_attrs
                                .iter()
                                .find(|(k, _)| k == "class")
                                .and_then(|(_, v)| {
                                    // Extract language from class like "language-typescript"
                                    v.strip_prefix("language-").map(|s| s.to_string())
                                });
                        let code = self.parse_text();
                        self.parse_closing_tag("code");
                        self.parse_closing_tag("pre");
                        return Some(HtmlElement::CodeBlock { language, code });
                    }
                }
                // No code tag, just pre content
                self.pos = saved_pos;
                let code = self.parse_text();
                self.parse_closing_tag("pre");
                Some(HtmlElement::CodeBlock {
                    language: None,
                    code,
                })
            }
            "ul" => {
                let children = self.parse_children("ul");
                Some(HtmlElement::UnorderedList(children))
            }
            "ol" => {
                let children = self.parse_children("ol");
                Some(HtmlElement::OrderedList(children))
            }
            "li" => {
                let children = self.parse_children("li");
                Some(HtmlElement::ListItem(children))
            }
            "blockquote" => {
                let children = self.parse_children("blockquote");
                Some(HtmlElement::Blockquote(children))
            }
            "a" => {
                let href = attributes
                    .iter()
                    .find(|(k, _)| k == "href")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                let children = self.parse_children("a");
                Some(HtmlElement::Link { href, children })
            }
            "div" => {
                let classes = attributes
                    .iter()
                    .find(|(k, _)| k == "class")
                    .map(|(_, v)| v.clone());
                let children = self.parse_children("div");
                Some(HtmlElement::Div { classes, children })
            }
            "span" => {
                let classes = attributes
                    .iter()
                    .find(|(k, _)| k == "class")
                    .map(|(_, v)| v.clone());
                let children = self.parse_children("span");
                Some(HtmlElement::Span { classes, children })
            }
            _ => {
                // Unknown tag - try to parse as container
                let children = self.parse_children(&tag_name);
                if children.is_empty() {
                    None
                } else if children.len() == 1 {
                    Some(children.into_iter().next().unwrap())
                } else {
                    Some(HtmlElement::Div {
                        classes: None,
                        children,
                    })
                }
            }
        }
    }

    /// Parse the entire HTML document
    fn parse(&mut self) -> Vec<HtmlElement> {
        let mut elements = Vec::new();

        while !self.is_eof() {
            self.skip_whitespace();
            if let Some(element) = self.parse_element() {
                elements.push(element);
            } else if !self.is_eof() {
                // Skip any remaining content we can't parse
                let text = self.parse_text();
                if !text.trim().is_empty() {
                    elements.push(HtmlElement::Text(text));
                }
            }
        }

        elements
    }
}

/// Decode common HTML entities
fn decode_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&ndash;", "\u{2013}")
        .replace("&mdash;", "\u{2014}")
        .replace("&copy;", "\u{00A9}")
        .replace("&reg;", "\u{00AE}")
        .replace("&trade;", "\u{2122}")
        .replace("&hellip;", "\u{2026}")
        .replace("&lsquo;", "\u{2018}")
        .replace("&rsquo;", "\u{2019}")
        .replace("&ldquo;", "\u{201C}")
        .replace("&rdquo;", "\u{201D}")
}

/// Parse HTML string into a vector of HtmlElement
///
/// # Examples
///
/// ```
/// use script_kit_gpui::utils::parse_html;
///
/// let elements = parse_html("<h1>Title</h1><p>Content</p>");
/// assert_eq!(elements.len(), 2);
/// ```
pub fn parse_html(html: &str) -> Vec<HtmlElement> {
    let mut parser = HtmlParser::new(html);
    parser.parse()
}

/// Extract plain text from parsed HTML elements
#[allow(dead_code)]
pub fn elements_to_text(elements: &[HtmlElement]) -> String {
    let mut result = String::new();

    for element in elements {
        match element {
            HtmlElement::Text(text) => {
                result.push_str(text);
            }
            HtmlElement::Header { children, .. }
            | HtmlElement::Paragraph(children)
            | HtmlElement::Bold(children)
            | HtmlElement::Italic(children)
            | HtmlElement::ListItem(children)
            | HtmlElement::Blockquote(children)
            | HtmlElement::Div { children, .. }
            | HtmlElement::Span { children, .. }
            | HtmlElement::Link { children, .. } => {
                result.push_str(&elements_to_text(children));
            }
            HtmlElement::UnorderedList(children) | HtmlElement::OrderedList(children) => {
                for child in children {
                    if let HtmlElement::ListItem(li_children) = child {
                        result.push_str(&elements_to_text(li_children));
                        result.push('\n');
                    }
                }
            }
            HtmlElement::InlineCode(code) | HtmlElement::CodeBlock { code, .. } => {
                result.push_str(code);
            }
            HtmlElement::HorizontalRule => {
                result.push_str("\n---\n");
            }
            HtmlElement::LineBreak => {
                result.push('\n');
            }
        }
    }

    result
}

// ============================================================================
// Asset Path Resolution
// ============================================================================

/// Get the path to a bundled asset that works both in development and in release builds.
///
/// In development (cargo run), assets are at `CARGO_MANIFEST_DIR/assets/`.
/// In release builds (.app bundle), assets are at `APP_BUNDLE/Contents/Resources/assets/`.
///
/// # Arguments
/// * `relative_path` - Path relative to the assets directory (e.g., "logo.svg" or "icons/check.svg")
///
/// # Returns
/// The full path to the asset as a String, suitable for use with GPUI's `svg().external_path()`.
pub fn get_asset_path(relative_path: &str) -> String {
    // First, try to find the asset in the app bundle (for release builds)
    #[cfg(target_os = "macos")]
    {
        if let Some(bundle_path) = get_macos_bundle_resources_path() {
            let asset_path = format!("{}/assets/{}", bundle_path, relative_path);
            if std::path::Path::new(&asset_path).exists() {
                return asset_path;
            }
        }
    }

    // Fall back to CARGO_MANIFEST_DIR for development builds
    // This is set at compile time, so it works when running via `cargo run`
    let dev_path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/");
    format!("{}{}", dev_path, relative_path)
}

/// Get the macOS app bundle's Resources directory path.
/// Returns None if not running from an app bundle.
#[cfg(target_os = "macos")]
fn get_macos_bundle_resources_path() -> Option<String> {
    // Get the path to the current executable
    let exe_path = std::env::current_exe().ok()?;

    // Check if we're in an app bundle structure:
    // /path/to/App.app/Contents/MacOS/executable
    let exe_dir = exe_path.parent()?; // Contents/MacOS
    let contents_dir = exe_dir.parent()?; // Contents

    // Verify this looks like a bundle
    if contents_dir.file_name()?.to_str()? != "Contents" {
        return None;
    }

    let resources_dir = contents_dir.join("Resources");
    if resources_dir.exists() {
        return resources_dir.to_str().map(|s| s.to_string());
    }

    None
}

/// Convenience function to get the logo.svg path
pub fn get_logo_path() -> String {
    get_asset_path("logo.svg")
}

// ============================================================================
// Tailwind CSS Class Mapping
// ============================================================================

/// Tailwind-style value that can be applied to a GPUI div
#[derive(Debug, Clone, Default)]
pub struct TailwindStyles {
    // Layout
    pub flex: bool,
    pub flex_col: bool,
    pub flex_row: bool,
    pub flex_1: bool,
    pub items_center: bool,
    pub items_start: bool,
    pub items_end: bool,
    pub justify_center: bool,
    pub justify_between: bool,
    pub justify_start: bool,
    pub justify_end: bool,

    // Sizing
    pub w_full: bool,
    pub h_full: bool,
    pub min_w_0: bool,
    pub min_h_0: bool,

    // Spacing (in pixels)
    pub padding: Option<f32>,
    pub padding_x: Option<f32>,
    pub padding_y: Option<f32>,
    pub padding_top: Option<f32>,
    pub padding_bottom: Option<f32>,
    pub padding_left: Option<f32>,
    pub padding_right: Option<f32>,
    pub margin: Option<f32>,
    pub margin_x: Option<f32>,
    pub margin_y: Option<f32>,
    pub margin_top: Option<f32>,
    pub margin_bottom: Option<f32>,
    pub margin_left: Option<f32>,
    pub margin_right: Option<f32>,
    pub gap: Option<f32>,

    // Colors (as 0xRRGGBB)
    pub text_color: Option<u32>,
    pub bg_color: Option<u32>,
    pub border_color: Option<u32>,

    // Typography
    pub font_size: Option<f32>,
    pub font_bold: bool,
    pub font_medium: bool,
    pub font_normal: bool,

    // Borders
    pub rounded: Option<f32>,
    pub border: bool,
    pub border_width: Option<f32>,
}

impl TailwindStyles {
    /// Parse a space-separated class string into TailwindStyles
    pub fn parse(class_string: &str) -> Self {
        let mut styles = TailwindStyles::default();

        for class in class_string.split_whitespace() {
            styles.apply_class(class);
        }

        styles
    }

    /// Apply a single Tailwind class to this style struct
    fn apply_class(&mut self, class: &str) {
        match class {
            // Layout
            "flex" => self.flex = true,
            "flex-col" => self.flex_col = true,
            "flex-row" => self.flex_row = true,
            "flex-1" => self.flex_1 = true,
            "items-center" => self.items_center = true,
            "items-start" => self.items_start = true,
            "items-end" => self.items_end = true,
            "justify-center" => self.justify_center = true,
            "justify-between" => self.justify_between = true,
            "justify-start" => self.justify_start = true,
            "justify-end" => self.justify_end = true,

            // Sizing
            "w-full" => self.w_full = true,
            "h-full" => self.h_full = true,
            "min-w-0" => self.min_w_0 = true,
            "min-h-0" => self.min_h_0 = true,

            // Typography
            "font-bold" => self.font_bold = true,
            "font-medium" => self.font_medium = true,
            "font-normal" => self.font_normal = true,
            "text-sm" => self.font_size = Some(14.0),
            "text-base" => self.font_size = Some(16.0),
            "text-lg" => self.font_size = Some(18.0),
            "text-xl" => self.font_size = Some(20.0),
            "text-2xl" => self.font_size = Some(24.0),
            "text-3xl" => self.font_size = Some(30.0),
            "text-4xl" => self.font_size = Some(36.0),

            // Border radius
            "rounded" => self.rounded = Some(4.0),
            "rounded-sm" => self.rounded = Some(2.0),
            "rounded-md" => self.rounded = Some(6.0),
            "rounded-lg" => self.rounded = Some(8.0),
            "rounded-xl" => self.rounded = Some(12.0),
            "rounded-2xl" => self.rounded = Some(16.0),
            "rounded-full" => self.rounded = Some(9999.0),
            "rounded-none" => self.rounded = Some(0.0),

            // Border
            "border" => self.border = true,
            "border-0" => self.border_width = Some(0.0),
            "border-2" => self.border_width = Some(2.0),
            "border-4" => self.border_width = Some(4.0),
            "border-8" => self.border_width = Some(8.0),

            // Otherwise, try pattern matching
            _ => self.apply_pattern_class(class),
        }
    }

    /// Apply classes that follow patterns like p-4, bg-blue-500, etc.
    fn apply_pattern_class(&mut self, class: &str) {
        // Spacing: p-*, px-*, py-*, pt-*, pb-*, pl-*, pr-*
        // Tailwind scale: 0=0, 1=4px, 2=8px, 3=12px, 4=16px, 5=20px, 6=24px, 8=32px, 10=40px, 12=48px
        if let Some(value) = class.strip_prefix("p-") {
            if let Some(px) = parse_spacing_value(value) {
                self.padding = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("px-") {
            if let Some(px) = parse_spacing_value(value) {
                self.padding_x = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("py-") {
            if let Some(px) = parse_spacing_value(value) {
                self.padding_y = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("pt-") {
            if let Some(px) = parse_spacing_value(value) {
                self.padding_top = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("pb-") {
            if let Some(px) = parse_spacing_value(value) {
                self.padding_bottom = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("pl-") {
            if let Some(px) = parse_spacing_value(value) {
                self.padding_left = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("pr-") {
            if let Some(px) = parse_spacing_value(value) {
                self.padding_right = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("m-") {
            if let Some(px) = parse_spacing_value(value) {
                self.margin = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("mx-") {
            if let Some(px) = parse_spacing_value(value) {
                self.margin_x = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("my-") {
            if let Some(px) = parse_spacing_value(value) {
                self.margin_y = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("mt-") {
            if let Some(px) = parse_spacing_value(value) {
                self.margin_top = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("mb-") {
            if let Some(px) = parse_spacing_value(value) {
                self.margin_bottom = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("ml-") {
            if let Some(px) = parse_spacing_value(value) {
                self.margin_left = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("mr-") {
            if let Some(px) = parse_spacing_value(value) {
                self.margin_right = Some(px);
            }
        } else if let Some(value) = class.strip_prefix("gap-") {
            if let Some(px) = parse_spacing_value(value) {
                self.gap = Some(px);
            }
        }
        // Text colors
        else if let Some(color_name) = class.strip_prefix("text-") {
            if let Some(color) = parse_color(color_name) {
                self.text_color = Some(color);
            }
        }
        // Background colors
        else if let Some(color_name) = class.strip_prefix("bg-") {
            if let Some(color) = parse_color(color_name) {
                self.bg_color = Some(color);
            }
        }
        // Border colors
        else if let Some(color_name) = class.strip_prefix("border-") {
            // Skip border width classes that we already handled
            if !["0", "2", "4", "8"].contains(&color_name) {
                if let Some(color) = parse_color(color_name) {
                    self.border_color = Some(color);
                }
            }
        }
    }
}

/// Parse Tailwind spacing values (0-12 scale) to pixels
fn parse_spacing_value(value: &str) -> Option<f32> {
    match value {
        "0" => Some(0.0),
        "0.5" => Some(2.0),
        "1" => Some(4.0),
        "1.5" => Some(6.0),
        "2" => Some(8.0),
        "2.5" => Some(10.0),
        "3" => Some(12.0),
        "3.5" => Some(14.0),
        "4" => Some(16.0),
        "5" => Some(20.0),
        "6" => Some(24.0),
        "7" => Some(28.0),
        "8" => Some(32.0),
        "9" => Some(36.0),
        "10" => Some(40.0),
        "11" => Some(44.0),
        "12" => Some(48.0),
        "14" => Some(56.0),
        "16" => Some(64.0),
        "20" => Some(80.0),
        "24" => Some(96.0),
        "auto" => None, // Can't represent auto in fixed pixels
        _ => {
            // Try to parse arbitrary value like [20px]
            if value.starts_with('[') && value.ends_with(']') {
                let inner = &value[1..value.len() - 1];
                if let Some(px_value) = inner.strip_suffix("px") {
                    return px_value.parse().ok();
                }
            }
            None
        }
    }
}

/// Parse Tailwind color names to hex values
fn parse_color(color_name: &str) -> Option<u32> {
    // Basic colors
    match color_name {
        "white" => return Some(0xFFFFFF),
        "black" => return Some(0x000000),
        "transparent" => return Some(0x000000), // Note: transparency not fully supported
        "current" => return None,               // Can't resolve current color
        _ => {}
    }

    // Parse color-shade format like "blue-500", "gray-100"
    if let Some((color, shade)) = color_name.rsplit_once('-') {
        let shade: u32 = shade.parse().ok()?;

        // Tailwind color palette (subset of most common colors)
        return match color {
            "slate" => get_slate_color(shade),
            "gray" => get_gray_color(shade),
            "zinc" => get_zinc_color(shade),
            "neutral" => get_neutral_color(shade),
            "stone" => get_stone_color(shade),
            "red" => get_red_color(shade),
            "orange" => get_orange_color(shade),
            "amber" => get_amber_color(shade),
            "yellow" => get_yellow_color(shade),
            "lime" => get_lime_color(shade),
            "green" => get_green_color(shade),
            "emerald" => get_emerald_color(shade),
            "teal" => get_teal_color(shade),
            "cyan" => get_cyan_color(shade),
            "sky" => get_sky_color(shade),
            "blue" => get_blue_color(shade),
            "indigo" => get_indigo_color(shade),
            "violet" => get_violet_color(shade),
            "purple" => get_purple_color(shade),
            "fuchsia" => get_fuchsia_color(shade),
            "pink" => get_pink_color(shade),
            "rose" => get_rose_color(shade),
            _ => None,
        };
    }

    None
}

// Tailwind color palette functions
fn get_slate_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xF8FAFC),
        100 => Some(0xF1F5F9),
        200 => Some(0xE2E8F0),
        300 => Some(0xCBD5E1),
        400 => Some(0x94A3B8),
        500 => Some(0x64748B),
        600 => Some(0x475569),
        700 => Some(0x334155),
        800 => Some(0x1E293B),
        900 => Some(0x0F172A),
        950 => Some(0x020617),
        _ => None,
    }
}

fn get_gray_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xF9FAFB),
        100 => Some(0xF3F4F6),
        200 => Some(0xE5E7EB),
        300 => Some(0xD1D5DB),
        400 => Some(0x9CA3AF),
        500 => Some(0x6B7280),
        600 => Some(0x4B5563),
        700 => Some(0x374151),
        800 => Some(0x1F2937),
        900 => Some(0x111827),
        950 => Some(0x030712),
        _ => None,
    }
}

fn get_zinc_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xFAFAFA),
        100 => Some(0xF4F4F5),
        200 => Some(0xE4E4E7),
        300 => Some(0xD4D4D8),
        400 => Some(0xA1A1AA),
        500 => Some(0x71717A),
        600 => Some(0x52525B),
        700 => Some(0x3F3F46),
        800 => Some(0x27272A),
        900 => Some(0x18181B),
        950 => Some(0x09090B),
        _ => None,
    }
}

fn get_neutral_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xFAFAFA),
        100 => Some(0xF5F5F5),
        200 => Some(0xE5E5E5),
        300 => Some(0xD4D4D4),
        400 => Some(0xA3A3A3),
        500 => Some(0x737373),
        600 => Some(0x525252),
        700 => Some(0x404040),
        800 => Some(0x262626),
        900 => Some(0x171717),
        950 => Some(0x0A0A0A),
        _ => None,
    }
}

fn get_stone_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xFAFAF9),
        100 => Some(0xF5F5F4),
        200 => Some(0xE7E5E4),
        300 => Some(0xD6D3D1),
        400 => Some(0xA8A29E),
        500 => Some(0x78716C),
        600 => Some(0x57534E),
        700 => Some(0x44403C),
        800 => Some(0x292524),
        900 => Some(0x1C1917),
        950 => Some(0x0C0A09),
        _ => None,
    }
}

fn get_red_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xFEF2F2),
        100 => Some(0xFEE2E2),
        200 => Some(0xFECACA),
        300 => Some(0xFCA5A5),
        400 => Some(0xF87171),
        500 => Some(0xEF4444),
        600 => Some(0xDC2626),
        700 => Some(0xB91C1C),
        800 => Some(0x991B1B),
        900 => Some(0x7F1D1D),
        950 => Some(0x450A0A),
        _ => None,
    }
}

fn get_orange_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xFFF7ED),
        100 => Some(0xFFEDD5),
        200 => Some(0xFED7AA),
        300 => Some(0xFDBA74),
        400 => Some(0xFB923C),
        500 => Some(0xF97316),
        600 => Some(0xEA580C),
        700 => Some(0xC2410C),
        800 => Some(0x9A3412),
        900 => Some(0x7C2D12),
        950 => Some(0x431407),
        _ => None,
    }
}

fn get_amber_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xFFFBEB),
        100 => Some(0xFEF3C7),
        200 => Some(0xFDE68A),
        300 => Some(0xFCD34D),
        400 => Some(0xFBBF24),
        500 => Some(0xF59E0B),
        600 => Some(0xD97706),
        700 => Some(0xB45309),
        800 => Some(0x92400E),
        900 => Some(0x78350F),
        950 => Some(0x451A03),
        _ => None,
    }
}

fn get_yellow_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xFEFCE8),
        100 => Some(0xFEF9C3),
        200 => Some(0xFEF08A),
        300 => Some(0xFDE047),
        400 => Some(0xFACC15),
        500 => Some(0xEAB308),
        600 => Some(0xCA8A04),
        700 => Some(0xA16207),
        800 => Some(0x854D0E),
        900 => Some(0x713F12),
        950 => Some(0x422006),
        _ => None,
    }
}

fn get_lime_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xF7FEE7),
        100 => Some(0xECFCCB),
        200 => Some(0xD9F99D),
        300 => Some(0xBEF264),
        400 => Some(0xA3E635),
        500 => Some(0x84CC16),
        600 => Some(0x65A30D),
        700 => Some(0x4D7C0F),
        800 => Some(0x3F6212),
        900 => Some(0x365314),
        950 => Some(0x1A2E05),
        _ => None,
    }
}

fn get_green_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xF0FDF4),
        100 => Some(0xDCFCE7),
        200 => Some(0xBBF7D0),
        300 => Some(0x86EFAC),
        400 => Some(0x4ADE80),
        500 => Some(0x22C55E),
        600 => Some(0x16A34A),
        700 => Some(0x15803D),
        800 => Some(0x166534),
        900 => Some(0x14532D),
        950 => Some(0x052E16),
        _ => None,
    }
}

fn get_emerald_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xECFDF5),
        100 => Some(0xD1FAE5),
        200 => Some(0xA7F3D0),
        300 => Some(0x6EE7B7),
        400 => Some(0x34D399),
        500 => Some(0x10B981),
        600 => Some(0x059669),
        700 => Some(0x047857),
        800 => Some(0x065F46),
        900 => Some(0x064E3B),
        950 => Some(0x022C22),
        _ => None,
    }
}

fn get_teal_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xF0FDFA),
        100 => Some(0xCCFBF1),
        200 => Some(0x99F6E4),
        300 => Some(0x5EEAD4),
        400 => Some(0x2DD4BF),
        500 => Some(0x14B8A6),
        600 => Some(0x0D9488),
        700 => Some(0x0F766E),
        800 => Some(0x115E59),
        900 => Some(0x134E4A),
        950 => Some(0x042F2E),
        _ => None,
    }
}

fn get_cyan_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xECFEFF),
        100 => Some(0xCFFAFE),
        200 => Some(0xA5F3FC),
        300 => Some(0x67E8F9),
        400 => Some(0x22D3EE),
        500 => Some(0x06B6D4),
        600 => Some(0x0891B2),
        700 => Some(0x0E7490),
        800 => Some(0x155E75),
        900 => Some(0x164E63),
        950 => Some(0x083344),
        _ => None,
    }
}

fn get_sky_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xF0F9FF),
        100 => Some(0xE0F2FE),
        200 => Some(0xBAE6FD),
        300 => Some(0x7DD3FC),
        400 => Some(0x38BDF8),
        500 => Some(0x0EA5E9),
        600 => Some(0x0284C7),
        700 => Some(0x0369A1),
        800 => Some(0x075985),
        900 => Some(0x0C4A6E),
        950 => Some(0x082F49),
        _ => None,
    }
}

fn get_blue_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xEFF6FF),
        100 => Some(0xDBEAFE),
        200 => Some(0xBFDBFE),
        300 => Some(0x93C5FD),
        400 => Some(0x60A5FA),
        500 => Some(0x3B82F6),
        600 => Some(0x2563EB),
        700 => Some(0x1D4ED8),
        800 => Some(0x1E40AF),
        900 => Some(0x1E3A8A),
        950 => Some(0x172554),
        _ => None,
    }
}

fn get_indigo_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xEEF2FF),
        100 => Some(0xE0E7FF),
        200 => Some(0xC7D2FE),
        300 => Some(0xA5B4FC),
        400 => Some(0x818CF8),
        500 => Some(0x6366F1),
        600 => Some(0x4F46E5),
        700 => Some(0x4338CA),
        800 => Some(0x3730A3),
        900 => Some(0x312E81),
        950 => Some(0x1E1B4B),
        _ => None,
    }
}

fn get_violet_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xF5F3FF),
        100 => Some(0xEDE9FE),
        200 => Some(0xDDD6FE),
        300 => Some(0xC4B5FD),
        400 => Some(0xA78BFA),
        500 => Some(0x8B5CF6),
        600 => Some(0x7C3AED),
        700 => Some(0x6D28D9),
        800 => Some(0x5B21B6),
        900 => Some(0x4C1D95),
        950 => Some(0x2E1065),
        _ => None,
    }
}

fn get_purple_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xFAF5FF),
        100 => Some(0xF3E8FF),
        200 => Some(0xE9D5FF),
        300 => Some(0xD8B4FE),
        400 => Some(0xC084FC),
        500 => Some(0xA855F7),
        600 => Some(0x9333EA),
        700 => Some(0x7E22CE),
        800 => Some(0x6B21A8),
        900 => Some(0x581C87),
        950 => Some(0x3B0764),
        _ => None,
    }
}

fn get_fuchsia_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xFDF4FF),
        100 => Some(0xFAE8FF),
        200 => Some(0xF5D0FE),
        300 => Some(0xF0ABFC),
        400 => Some(0xE879F9),
        500 => Some(0xD946EF),
        600 => Some(0xC026D3),
        700 => Some(0xA21CAF),
        800 => Some(0x86198F),
        900 => Some(0x701A75),
        950 => Some(0x4A044E),
        _ => None,
    }
}

fn get_pink_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xFDF2F8),
        100 => Some(0xFCE7F3),
        200 => Some(0xFBCFE8),
        300 => Some(0xF9A8D4),
        400 => Some(0xF472B6),
        500 => Some(0xEC4899),
        600 => Some(0xDB2777),
        700 => Some(0xBE185D),
        800 => Some(0x9D174D),
        900 => Some(0x831843),
        950 => Some(0x500724),
        _ => None,
    }
}

fn get_rose_color(shade: u32) -> Option<u32> {
    match shade {
        50 => Some(0xFFF1F2),
        100 => Some(0xFFE4E6),
        200 => Some(0xFECDD3),
        300 => Some(0xFDA4AF),
        400 => Some(0xFB7185),
        500 => Some(0xF43F5E),
        600 => Some(0xE11D48),
        700 => Some(0xBE123C),
        800 => Some(0x9F1239),
        900 => Some(0x881337),
        950 => Some(0x4C0519),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // strip_html_tags tests (existing)
    // ========================================================================

    #[test]
    fn test_basic_tag_removal() {
        assert_eq!(strip_html_tags("<p>Hello</p>"), "Hello");
    }

    #[test]
    fn test_nested_tags() {
        assert_eq!(
            strip_html_tags("<div><span>A</span><span>B</span></div>"),
            "A B"
        );
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(strip_html_tags(""), "");
    }

    #[test]
    fn test_no_tags() {
        assert_eq!(strip_html_tags("plain text"), "plain text");
    }

    #[test]
    fn test_whitespace_normalization() {
        assert_eq!(strip_html_tags("<p>Hello   World</p>"), "Hello World");
    }

    #[test]
    fn test_multiple_tags_with_content() {
        assert_eq!(
            strip_html_tags("<h1>Title</h1><p>Paragraph</p>"),
            "Title Paragraph"
        );
    }

    #[test]
    fn test_self_closing_tags() {
        assert_eq!(strip_html_tags("Hello<br/>World"), "Hello World");
    }

    #[test]
    fn test_tags_with_attributes() {
        assert_eq!(
            strip_html_tags("<a href=\"https://example.com\">Link</a>"),
            "Link"
        );
    }

    #[test]
    fn test_deeply_nested() {
        assert_eq!(
            strip_html_tags("<div><div><div>Deep</div></div></div>"),
            "Deep"
        );
    }

    #[test]
    fn test_only_tags() {
        assert_eq!(strip_html_tags("<div><span></span></div>"), "");
    }

    #[test]
    fn test_leading_trailing_whitespace() {
        assert_eq!(strip_html_tags("  <p>  Hello  </p>  "), "Hello");
    }

    #[test]
    fn test_newlines_in_html() {
        assert_eq!(
            strip_html_tags("<p>\n  Hello\n  World\n</p>"),
            "Hello World"
        );
    }

    // ========================================================================
    // HTML parsing tests
    // ========================================================================

    #[test]
    fn test_parse_simple_text() {
        let elements = parse_html("Hello World");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], HtmlElement::Text(t) if t == "Hello World"));
    }

    #[test]
    fn test_parse_paragraph() {
        let elements = parse_html("<p>Hello</p>");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            HtmlElement::Paragraph(children) => {
                assert_eq!(children.len(), 1);
                assert!(matches!(&children[0], HtmlElement::Text(t) if t == "Hello"));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn test_parse_headers() {
        for level in 1..=6u8 {
            let html = format!("<h{}>Header {}</h{}>", level, level, level);
            let elements = parse_html(&html);
            assert_eq!(elements.len(), 1);
            match &elements[0] {
                HtmlElement::Header { level: l, children } => {
                    assert_eq!(*l, level);
                    assert_eq!(children.len(), 1);
                }
                _ => panic!("Expected Header level {}", level),
            }
        }
    }

    #[test]
    fn test_parse_bold() {
        let elements = parse_html("<strong>Bold</strong>");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], HtmlElement::Bold(_)));

        let elements = parse_html("<b>Also Bold</b>");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], HtmlElement::Bold(_)));
    }

    #[test]
    fn test_parse_italic() {
        let elements = parse_html("<em>Italic</em>");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], HtmlElement::Italic(_)));

        let elements = parse_html("<i>Also Italic</i>");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], HtmlElement::Italic(_)));
    }

    #[test]
    fn test_parse_inline_code() {
        let elements = parse_html("<code>let x = 42;</code>");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], HtmlElement::InlineCode(c) if c == "let x = 42;"));
    }

    #[test]
    fn test_parse_code_block() {
        let elements =
            parse_html("<pre><code class=\"language-typescript\">const x = 1;</code></pre>");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            HtmlElement::CodeBlock { language, code } => {
                assert_eq!(language.as_deref(), Some("typescript"));
                assert_eq!(code, "const x = 1;");
            }
            _ => panic!("Expected CodeBlock"),
        }
    }

    #[test]
    fn test_parse_unordered_list() {
        let elements = parse_html("<ul><li>Item 1</li><li>Item 2</li></ul>");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            HtmlElement::UnorderedList(items) => {
                assert_eq!(items.len(), 2);
            }
            _ => panic!("Expected UnorderedList"),
        }
    }

    #[test]
    fn test_parse_ordered_list() {
        let elements = parse_html("<ol><li>First</li><li>Second</li></ol>");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            HtmlElement::OrderedList(items) => {
                assert_eq!(items.len(), 2);
            }
            _ => panic!("Expected OrderedList"),
        }
    }

    #[test]
    fn test_parse_blockquote() {
        let elements = parse_html("<blockquote>Quote</blockquote>");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], HtmlElement::Blockquote(_)));
    }

    #[test]
    fn test_parse_link() {
        let elements = parse_html("<a href=\"https://example.com\">Link</a>");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            HtmlElement::Link { href, children } => {
                assert_eq!(href, "https://example.com");
                assert_eq!(children.len(), 1);
            }
            _ => panic!("Expected Link"),
        }
    }

    #[test]
    fn test_parse_hr() {
        let elements = parse_html("<hr>");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], HtmlElement::HorizontalRule));

        let elements = parse_html("<hr/>");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], HtmlElement::HorizontalRule));
    }

    #[test]
    fn test_parse_br() {
        let elements = parse_html("Line1<br>Line2");
        assert_eq!(elements.len(), 3);
        assert!(matches!(&elements[0], HtmlElement::Text(t) if t == "Line1"));
        assert!(matches!(&elements[1], HtmlElement::LineBreak));
        assert!(matches!(&elements[2], HtmlElement::Text(t) if t == "Line2"));
    }

    #[test]
    fn test_parse_nested_elements() {
        let elements = parse_html("<p><strong>Bold</strong> and <em>italic</em></p>");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            HtmlElement::Paragraph(children) => {
                // Parser produces: Bold, " and ", Italic (3 children)
                assert_eq!(children.len(), 3);
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn test_parse_html_entities() {
        let elements = parse_html("<p>&amp; &lt; &gt; &quot;</p>");
        match &elements[0] {
            HtmlElement::Paragraph(children) => match &children[0] {
                HtmlElement::Text(t) => {
                    assert!(t.contains('&'));
                    assert!(t.contains('<'));
                    assert!(t.contains('>'));
                    assert!(t.contains('"'));
                }
                _ => panic!("Expected Text"),
            },
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn test_elements_to_text() {
        let elements = parse_html("<h1>Title</h1><p>Paragraph with <strong>bold</strong></p>");
        let text = elements_to_text(&elements);
        assert!(text.contains("Title"));
        assert!(text.contains("Paragraph"));
        assert!(text.contains("bold"));
    }

    #[test]
    fn test_parse_complex_html() {
        let html = r#"
            <h1>Welcome</h1>
            <p>This is a <strong>test</strong> with <em>formatting</em>.</p>
            <ul>
                <li>Item one</li>
                <li>Item two</li>
            </ul>
            <blockquote>A quote</blockquote>
            <pre><code>let x = 1;</code></pre>
        "#;
        let elements = parse_html(html);
        // Should parse without panicking and produce multiple elements
        assert!(elements.len() >= 4);
    }

    #[test]
    fn test_case_insensitive_tags() {
        let elements = parse_html("<P>Paragraph</P>");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], HtmlElement::Paragraph(_)));

        let elements = parse_html("<STRONG>Bold</STRONG>");
        assert_eq!(elements.len(), 1);
        assert!(matches!(&elements[0], HtmlElement::Bold(_)));
    }

    // ========================================================================
    // Tailwind CSS parsing tests
    // ========================================================================

    #[test]
    fn test_tailwind_flex_classes() {
        let styles = TailwindStyles::parse("flex flex-col items-center justify-between");
        assert!(styles.flex);
        assert!(styles.flex_col);
        assert!(styles.items_center);
        assert!(styles.justify_between);
    }

    #[test]
    fn test_tailwind_spacing_classes() {
        let styles = TailwindStyles::parse("p-4 px-2 mt-8 gap-2");
        assert_eq!(styles.padding, Some(16.0));
        assert_eq!(styles.padding_x, Some(8.0));
        assert_eq!(styles.margin_top, Some(32.0));
        assert_eq!(styles.gap, Some(8.0));
    }

    #[test]
    fn test_tailwind_color_classes() {
        let styles = TailwindStyles::parse("text-white bg-blue-500 border-gray-300");
        assert_eq!(styles.text_color, Some(0xFFFFFF));
        assert_eq!(styles.bg_color, Some(0x3B82F6));
        assert_eq!(styles.border_color, Some(0xD1D5DB));
    }

    #[test]
    fn test_tailwind_typography_classes() {
        let styles = TailwindStyles::parse("text-2xl font-bold");
        assert_eq!(styles.font_size, Some(24.0));
        assert!(styles.font_bold);
    }

    #[test]
    fn test_tailwind_border_radius_classes() {
        let styles = TailwindStyles::parse("rounded-lg");
        assert_eq!(styles.rounded, Some(8.0));

        let styles = TailwindStyles::parse("rounded-full");
        assert_eq!(styles.rounded, Some(9999.0));
    }

    #[test]
    fn test_tailwind_sizing_classes() {
        let styles = TailwindStyles::parse("w-full h-full min-w-0");
        assert!(styles.w_full);
        assert!(styles.h_full);
        assert!(styles.min_w_0);
    }

    #[test]
    fn test_parse_div_with_classes() {
        let elements = parse_html("<div class=\"p-4 bg-blue-500 text-white\">Content</div>");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            HtmlElement::Div { classes, children } => {
                assert_eq!(classes.as_deref(), Some("p-4 bg-blue-500 text-white"));
                assert_eq!(children.len(), 1);
            }
            _ => panic!("Expected Div with classes"),
        }
    }

    #[test]
    fn test_parse_span_with_classes() {
        let elements = parse_html("<span class=\"font-bold text-red-500\">Bold red</span>");
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            HtmlElement::Span { classes, children } => {
                assert_eq!(classes.as_deref(), Some("font-bold text-red-500"));
                assert_eq!(children.len(), 1);
            }
            _ => panic!("Expected Span with classes"),
        }
    }
}

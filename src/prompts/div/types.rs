use super::*;

/// Options for customizing the div container appearance
#[derive(Debug, Clone, Default)]
pub struct ContainerOptions {
    /// Background color: "transparent", "#RRGGBB", "#RRGGBBAA", or Tailwind color name
    pub background: Option<String>,
    /// Padding in pixels, or None to use default
    pub padding: Option<ContainerPadding>,
    /// Opacity (0-100), applies to the container background color
    pub opacity: Option<u8>,
    /// Tailwind classes for the content container
    pub container_classes: Option<String>,
}

/// Padding options for the container
#[derive(Debug, Clone)]
pub enum ContainerPadding {
    /// No padding
    None,
    /// Custom padding in pixels
    Pixels(f32),
}

impl ContainerOptions {
    /// Parse container background to GPUI color
    pub fn parse_background(&self) -> Option<Hsla> {
        let bg = self.background.as_ref()?;

        // Handle "transparent"
        if bg == "transparent" {
            return Some(Hsla::transparent_black());
        }

        // Handle hex colors: #RGB, #RRGGBB, #RRGGBBAA
        if bg.starts_with('#') {
            return parse_hex_color(bg);
        }

        // Handle Tailwind color names (e.g., "blue-500", "gray-900")
        if let Some(color) = parse_color(bg) {
            return Some(rgb_to_hsla(color, self.opacity));
        }

        None
    }

    /// Get padding value
    pub fn get_padding(&self, default: f32) -> f32 {
        match &self.padding {
            Some(ContainerPadding::None) => 0.0,
            Some(ContainerPadding::Pixels(px)) => *px,
            None => default,
        }
    }
}

/// Parse hex color string to GPUI Hsla
fn parse_hex_color(hex: &str) -> Option<Hsla> {
    let hex = hex.trim_start_matches('#');

    // A hex color is ASCII-only. Reject non-ASCII up front so the byte-length
    // `match` below and its fixed-offset slices (`&hex[0..1]`, `&hex[0..2]`, …)
    // can never land inside a multibyte char and panic.
    if !hex.is_ascii() {
        return None;
    }

    match hex.len() {
        // #RGB -> #RRGGBB
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some(Hsla::from(gpui::Rgba {
                r: r as f32 / 255.0,
                g: g as f32 / 255.0,
                b: b as f32 / 255.0,
                a: 1.0,
            }))
        }
        // #RRGGBB
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Hsla::from(gpui::Rgba {
                r: r as f32 / 255.0,
                g: g as f32 / 255.0,
                b: b as f32 / 255.0,
                a: 1.0,
            }))
        }
        // #RRGGBBAA
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Hsla::from(gpui::Rgba {
                r: r as f32 / 255.0,
                g: g as f32 / 255.0,
                b: b as f32 / 255.0,
                a: a as f32 / 255.0,
            }))
        }
        _ => None,
    }
}

#[inline]
pub(super) fn is_div_submit_key(key: &str) -> bool {
    is_key_enter(key) || is_key_escape(key)
}

/// Convert RGB u32 to Hsla with optional opacity
pub(super) fn rgb_to_hsla(color: u32, opacity: Option<u8>) -> Hsla {
    let r = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b = (color & 0xFF) as f32 / 255.0;
    let a = opacity.map(|o| o as f32 / 100.0).unwrap_or(1.0);
    Hsla::from(gpui::Rgba { r, g, b, a })
}

#[inline]
pub(super) fn default_container_padding(variant: DesignVariant) -> f32 {
    get_tokens(variant).spacing().padding_md
}
/// Callback type for link clicks - needs App context to update entity
pub(super) type LinkClickCallback = Arc<dyn Fn(&str, &mut gpui::App) + Send + Sync>;

/// Style context for rendering HTML elements
#[derive(Clone)]
pub(super) struct RenderContext {
    /// Primary text color
    pub(super) text_primary: u32,
    /// Secondary text color (for muted content)
    pub(super) text_secondary: u32,
    /// Tertiary text color
    pub(super) text_tertiary: u32,
    /// Accent/link color
    pub(super) accent_color: u32,
    /// Code background color
    pub(super) code_bg: u32,
    /// Blockquote border color
    pub(super) quote_border: u32,
    /// HR color
    pub(super) hr_color: u32,
    /// Optional link click callback
    pub(super) on_link_click: Option<LinkClickCallback>,
}

impl RenderContext {
    pub(super) fn from_theme(colors: &theme::ColorScheme) -> Self {
        Self {
            text_primary: colors.text.primary,
            text_secondary: colors.text.secondary,
            text_tertiary: colors.text.tertiary,
            accent_color: colors.accent.selected,
            code_bg: colors.background.search_box,
            quote_border: colors.ui.border,
            hr_color: colors.ui.border,
            on_link_click: None,
        }
    }
}

#[cfg(test)]
mod hex_color_tests {
    use super::{parse_hex_color, ContainerOptions};

    #[test]
    fn parse_hex_color_rejects_multibyte_without_panicking() {
        // Regression: `match hex.len()` used the *byte* length, then sliced by
        // fixed byte offsets (`&hex[0..1]`, `&hex[0..2]`, …). A single 3-byte char
        // like `€` has `len() == 3`, hit the `#RGB` arm, and `&hex[0..1]` sliced
        // inside `€` → "byte index 1 is not a char boundary" panic. Non-ASCII must
        // just fail to parse (None), never crash.
        assert_eq!(parse_hex_color("€"), None); // 3 bytes, 1 char
        assert_eq!(parse_hex_color("#€"), None);
        assert_eq!(parse_hex_color("é€a"), None); // 6 bytes, mid-char at offset 2/4
        assert_eq!(parse_hex_color("€€"), None); // 6 bytes
        assert_eq!(parse_hex_color("aé€aa"), None); // 8 bytes
                                                    // Valid hex still parses.
        assert!(parse_hex_color("#fff").is_some());
        assert!(parse_hex_color("#ff0000").is_some());
        assert!(parse_hex_color("#ff000080").is_some());
        assert!(parse_hex_color("nothex").is_none());
    }

    #[test]
    fn parse_background_survives_multibyte_hex() {
        // Reached from a script-supplied container background string.
        let options = ContainerOptions {
            background: Some("#€".to_string()),
            ..Default::default()
        };
        assert_eq!(options.parse_background(), None);
    }
}

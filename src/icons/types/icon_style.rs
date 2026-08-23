//! Icon styling types

use gpui::Hsla;

/// Icon size presets
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum IconSize {
    /// 12px - Extra small
    XSmall,
    /// 14px - Small
    Small,
    /// 16px - Medium (default)
    #[default]
    Medium,
    /// 20px - Large
    Large,
    /// 24px - Extra large
    XLarge,
    /// Custom pixel size
    Custom(f32),
}

impl IconSize {
    /// Convert to pixel value
    pub fn to_px(self) -> f32 {
        match self {
            Self::XSmall => 12.0,
            Self::Small => 14.0,
            Self::Medium => 16.0,
            Self::Large => 20.0,
            Self::XLarge => 24.0,
            Self::Custom(px) => px,
        }
    }
}

/// Color tokens for themed icon colors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorToken {
    /// Primary foreground color
    Primary,
    /// Muted/secondary color
    Muted,
    /// Accent/highlight color
    Accent,
    /// Danger/error color
    Danger,
    /// Success/positive color
    Success,
    /// Warning color
    Warning,
}

/// Icon color specification
#[derive(Debug, Clone, Default)]
pub enum IconColor {
    /// Inherit color from parent text style
    #[default]
    Inherit,
    /// Use a theme color token
    Token(ColorToken),
    /// Use a fixed HSLA color
    Fixed(Hsla),
    /// Don't apply any tint (for full-color icons)
    None,
}

impl IconColor {
    /// Create a fixed color from hex (0xRRGGBB)
    pub fn from_hex(hex: u32) -> Self {
        Self::Fixed(gpui::rgb(hex).into())
    }
}

/// Icon style configuration
#[derive(Debug, Clone)]
pub struct IconStyle {
    /// Icon size
    pub size: IconSize,
    /// Icon color
    pub color: IconColor,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Rotation in radians
    pub rotation: Option<f32>,
}

impl Default for IconStyle {
    fn default() -> Self {
        Self {
            size: IconSize::default(),
            color: IconColor::default(),
            opacity: 1.0,
            rotation: None,
        }
    }
}

impl IconStyle {
    /// Set the size
    pub fn with_size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    /// Set the color
    pub fn with_color(mut self, color: IconColor) -> Self {
        self.color = color;
        self
    }

    /// Set the opacity
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Set the rotation in radians
    pub fn with_rotation(mut self, radians: f32) -> Self {
        self.rotation = Some(radians);
        self
    }
}

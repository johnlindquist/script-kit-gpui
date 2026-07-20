/// Pre-computed colors for form field rendering
///
/// This struct holds the color values needed for form field rendering,
/// allowing efficient use in closures without cloning the full theme.
#[derive(Clone, Copy, Debug)]
pub struct FormFieldColors {
    /// Background color of the input
    pub background: u32,
    /// Background color when focused
    pub background_focused: u32,
    /// Text color when typing.
    pub text: gpui::Rgba,
    /// Placeholder text color.
    pub placeholder: gpui::Rgba,
    /// Label text color.
    pub label: gpui::Rgba,
    /// Border color
    pub border: u32,
    /// Border color when focused
    pub border_focused: u32,
    /// Cursor color.
    pub cursor: gpui::Rgba,
    /// Checkbox checked background
    pub checkbox_checked: u32,
    /// Checkbox check mark color.
    pub checkbox_mark: gpui::Rgba,
    /// Shared input font size token for all editable field text
    pub input_font_size: f32,
    /// Shared label font size token for labels, hints, and inline indicators
    pub label_font_size: f32,
}

impl FormFieldColors {
    /// Create FormFieldColors from a Theme
    pub fn from_theme(theme: &crate::theme::Theme) -> Self {
        let ui_font_size = theme.get_fonts().ui_size;
        let chrome = crate::theme::AppChromeColors::from_theme(theme);
        Self {
            background: theme.colors.background.search_box,
            background_focused: theme.colors.background.main,
            text: gpui::rgb(chrome.text_primary_hex),
            placeholder: gpui::rgba(chrome.placeholder_text_rgba),
            label: gpui::rgba(chrome.text_muted_rgba),
            border: theme.colors.ui.border,
            border_focused: chrome.accent_hex,
            cursor: gpui::rgb(chrome.accent_hex),
            checkbox_checked: chrome.accent_hex,
            checkbox_mark: gpui::rgb(theme.colors.background.main),
            input_font_size: (ui_font_size + 2.0).max(12.0),
            label_font_size: (ui_font_size - 2.0).max(10.0),
        }
    }

    /// Create FormFieldColors from design colors
    pub fn from_design(colors: &crate::designs::DesignColors) -> Self {
        use crate::theme::opacity::{OPACITY_DISABLED, OPACITY_TEXT_MUTED};
        use crate::ui_foundation::hex_to_rgba_with_opacity;

        let typography = crate::designs::DesignTypography::default();
        Self {
            background: colors.background_secondary,
            background_focused: colors.background,
            text: gpui::rgb(colors.text_primary),
            placeholder: gpui::rgba(hex_to_rgba_with_opacity(
                colors.text_primary,
                OPACITY_DISABLED,
            )),
            label: gpui::rgba(hex_to_rgba_with_opacity(
                colors.text_primary,
                OPACITY_TEXT_MUTED,
            )),
            border: colors.border,
            border_focused: colors.accent,
            cursor: gpui::rgb(colors.accent),
            checkbox_checked: colors.accent,
            checkbox_mark: gpui::rgb(colors.background),
            input_font_size: typography.font_size_lg,
            label_font_size: typography.font_size_sm,
        }
    }
}

/// Pre-computed whisper-chrome surface for form fields.
#[derive(Clone, Copy, Debug)]
pub struct FormFieldSurface {
    /// Background color with low alpha.
    pub background: gpui::Rgba,
    /// Border color — accent on focus, ghost otherwise.
    pub border: gpui::Rgba,
    /// Accent fill used by checked controls on the same surface.
    pub checked_fill: gpui::Rgba,
}

impl FormFieldColors {
    /// Compute a whisper-chrome surface for a form field.
    /// Focused fields use `background_focused` at a slightly stronger opacity,
    /// while idle fields stay in ghost opacity.
    pub fn whisper_surface(&self, focused: bool) -> FormFieldSurface {
        use crate::theme::opacity::{
            OPACITY_WHISPER_ACCENT_FILL, OPACITY_WHISPER_BORDER_IDLE,
            OPACITY_WHISPER_SURFACE_FOCUSED, OPACITY_WHISPER_SURFACE_IDLE,
        };
        use crate::ui_foundation::hex_to_rgba_with_opacity;

        let background_hex = if focused {
            self.background_focused
        } else {
            self.background
        };
        let background_opacity = if focused {
            OPACITY_WHISPER_SURFACE_FOCUSED
        } else {
            OPACITY_WHISPER_SURFACE_IDLE
        };

        FormFieldSurface {
            background: gpui::rgba(hex_to_rgba_with_opacity(background_hex, background_opacity)),
            border: if focused {
                gpui::rgb(self.border_focused)
            } else {
                gpui::rgba(hex_to_rgba_with_opacity(
                    self.border,
                    OPACITY_WHISPER_BORDER_IDLE,
                ))
            },
            checked_fill: gpui::rgba(hex_to_rgba_with_opacity(
                self.checkbox_checked,
                OPACITY_WHISPER_ACCENT_FILL,
            )),
        }
    }
}

impl Default for FormFieldColors {
    fn default() -> Self {
        Self::from_theme(&crate::theme::Theme::default())
    }
}

/// Shared form-field dimensions and typography derived from theme/design tokens.
#[derive(Clone, Copy, Debug)]
pub struct FormFieldMetrics {
    /// Label font size used by labels, hints, and inline indicators.
    pub label_font_size: f32,
    /// Label line height.
    pub label_line_height: f32,
    /// Input/value font size used by editable field text.
    pub input_font_size: f32,
    /// Input/value line height.
    pub input_line_height: f32,
    /// Vertical gap between a label and its field surface.
    pub field_gap_px: f32,
    /// Horizontal padding inside a compact form field surface.
    pub field_padding_x_px: f32,
    /// Vertical padding inside a compact form field surface.
    pub field_padding_y_px: f32,
    /// Radius for compact form field surfaces.
    pub field_radius_px: f32,
    /// Horizontal gap between label-row items inside compact form field surfaces.
    pub field_header_gap_px: f32,
    /// Cursor width for general text field renderers.
    pub cursor_width_px: f32,
    /// Cursor height for general text field renderers.
    pub cursor_height_rems: f32,
    /// Height of one textarea row in rems.
    pub text_area_row_height_rems: f32,
    /// Additional textarea vertical padding in rems.
    pub text_area_vertical_padding_rems: f32,
    /// Checkbox square size in rems.
    pub checkbox_box_size_rems: f32,
    /// Gap between checkbox square and label in rems.
    pub checkbox_gap_rems: f32,
    /// Checkbox square radius in px.
    pub checkbox_radius_px: f32,
}

impl FormFieldMetrics {
    /// Minimum visible rows for multiline handler form inputs.
    pub const MULTILINE_MIN_ROWS: usize = 2;
    /// Maximum visible rows for multiline handler form inputs.
    pub const MULTILINE_MAX_ROWS: usize = 6;

    /// Resolve metrics from a theme and design variant.
    pub fn from_theme_and_design(
        theme: &crate::theme::Theme,
        design_variant: crate::designs::DesignVariant,
    ) -> Self {
        let colors = FormFieldColors::from_theme(theme);
        let typography = crate::designs::get_tokens(design_variant).typography();
        Self::from_font_sizes(
            colors.label_font_size,
            colors.input_font_size,
            typography.line_height_normal,
        )
    }

    /// Resolve metrics from an already materialized form color token set.
    pub fn from_colors(colors: FormFieldColors) -> Self {
        let typography = crate::designs::DesignTypography::default();
        Self::from_font_sizes(
            colors.label_font_size,
            colors.input_font_size,
            typography.line_height_normal,
        )
    }

    fn from_font_sizes(
        label_font_size: f32,
        input_font_size: f32,
        line_height_normal: f32,
    ) -> Self {
        Self {
            label_font_size,
            label_line_height: (label_font_size * line_height_normal).max(label_font_size + 4.0),
            input_font_size,
            input_line_height: (input_font_size * line_height_normal).max(input_font_size + 4.0),
            field_gap_px: 6.0,
            field_padding_x_px: 10.0,
            field_padding_y_px: 8.0,
            field_radius_px: 6.0,
            field_header_gap_px: 8.0,
            cursor_width_px: 2.0,
            cursor_height_rems: 1.125,
            text_area_row_height_rems: 1.5,
            text_area_vertical_padding_rems: 1.0,
            checkbox_box_size_rems: 1.125,
            checkbox_gap_rems: 0.75,
            checkbox_radius_px: 4.0,
        }
    }

    /// Fixed single-line input height used by menu-syntax handler fields.
    pub fn menu_syntax_single_line_height_px(&self) -> f32 {
        self.input_line_height + (crate::panel::CURSOR_MARGIN_Y * 2.0)
    }

    /// Rendered text size for menu-syntax `Input` fields.
    pub fn menu_syntax_input_rendered_font_size_px(&self) -> f32 {
        self.input_font_size * 0.875
    }

    /// Multiline min/max heights used by menu-syntax handler fields.
    pub fn menu_syntax_multiline_height_px(&self, rows: f32) -> f32 {
        (self.input_line_height * rows) + (crate::panel::CURSOR_MARGIN_Y * 2.0)
    }

    /// Minimum multiline handler input height in px.
    pub fn menu_syntax_multiline_min_height_px(&self) -> f32 {
        self.menu_syntax_multiline_height_px(Self::MULTILINE_MIN_ROWS as f32)
    }

    /// Maximum multiline handler input height in px.
    pub fn menu_syntax_multiline_max_height_px(&self) -> f32 {
        self.menu_syntax_multiline_height_px(Self::MULTILINE_MAX_ROWS as f32)
    }

    /// Height for general form textarea rows.
    pub fn text_area_height_rems(&self, rows: usize) -> f32 {
        (rows as f32) * self.text_area_row_height_rems + self.text_area_vertical_padding_rems
    }
}

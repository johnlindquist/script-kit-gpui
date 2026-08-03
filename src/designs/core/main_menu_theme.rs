use std::sync::atomic::{AtomicU8, Ordering};

use gpui::FontWeight;

pub const MAIN_MENU_HEADER_CONTEXT_EDGE_OUTSET_X: f32 = 8.0;
/// Canonical hover-button corner radius: the footer trailing action buttons
/// own this value, and every other hover-pill button (header context chips,
/// footer tip) must share it so all buttons read as one system.
pub const MAIN_MENU_ACTION_BUTTON_RADIUS: f32 = crate::ui::chrome::ACTION_BUTTON_RADIUS_PX;
/// Canonical keycap-border alpha while a hover-pill button is hovered.
/// Shared by the footer buttons and the header context chips.
pub const MAIN_MENU_ACTION_BUTTON_HOVER_BORDER_ALPHA: u32 =
    crate::ui::chrome::ACTION_BUTTON_HOVER_BORDER_ALPHA;
pub const MAIN_MENU_SECTION_PADDING_X: f32 = 14.0;
pub const MAIN_MENU_SECTION_PADDING_TOP: f32 = 12.0;
pub const MAIN_MENU_SECTION_PADDING_BOTTOM: f32 = 4.0;
pub const MAIN_MENU_SECTION_GAP: f32 = 6.0;
pub const MAIN_MENU_SECTION_ICON_SIZE: f32 = 10.0;
pub const MAIN_MENU_SECTION_WEIGHT: FontWeight = FontWeight::SEMIBOLD;
pub const MAIN_MENU_METADATA_SOURCE_FONT_SIZE: f32 = 11.0;
pub const MAIN_MENU_METADATA_BADGE_FONT_SIZE: f32 = 10.0;
pub const MAIN_MENU_METADATA_BADGE_PADDING_X: f32 = 4.0;
pub const MAIN_MENU_METADATA_BADGE_PADDING_Y: f32 = 1.0;
pub const MAIN_MENU_METADATA_BADGE_RADIUS: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum MainMenuThemeVariant {
    #[default]
    InfoBarBase = 1,
    InfoBarBreadcrumb = 2,
    InfoBarCompact = 3,
    InfoBarSplit = 4,
    InfoBarCwdFocus = 5,
    InfoBarModelFocus = 6,
    InfoBarSlashPath = 7,
    InfoBarMutedPills = 8,
    InfoBarPlainText = 9,
    InfoBarLowContrastKeys = 10,
    InfoBarStrongKeys = 11,
    InfoBarCentered = 12,
    InfoBarLeftDense = 13,
    InfoBarRightUtility = 14,
    InfoBarUltraQuiet = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuThemeTier {
    TahoeClose,
    ExpressiveTahoe,
    Exploratory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuRowKind {
    IconTile,
    GraphitePill,
    BlueGlass,
    Smoke,
    WarmGold,
    FrostedCommand,
    LiquidPrism,
    AuroraSlate,
    MilkGlass,
    ProConsole,
    SpotlightLuxe,
    OceanGlass,
    CarbonNeon,
    StudioPaperGlass,
    OperatorMonoGlass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FooterButtonTheme {
    pub border_alpha: u32,
    pub hover_border_alpha: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MainMenuShellTokens {
    pub content_inset_x: f32,
    pub content_inset_bottom: f32,
    pub header_padding_x: f32,
    pub header_padding_y: f32,
    pub header_gap: f32,
    pub divider_margin_x: f32,
    pub divider_height: f32,
    pub divider_alpha: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MainMenuSearchTokens {
    pub height: f32,
    pub radius: f32,
    pub text_inset_x: f32,
    pub text_inset_y: f32,
    pub surface_alpha: u32,
    pub border_alpha: u32,
    pub font_size: f32,
    pub font_weight: FontWeight,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MainMenuListTokens {
    pub item_height: f32,
    pub section_header_height: f32,
    pub first_section_header_height: f32,
    pub source_status_row_height: f32,
    pub average_scroll_height: f32,
    pub footer_reveal_clearance_height: f32,
    pub scrollbar_width: f32,
    /// Height of the fixed top occlusion plane shown once list content scrolls.
    pub top_occlusion_height: f32,
    /// Logical scroll distance over which the top occlusion reaches full strength.
    pub top_occlusion_ramp: f32,
    /// Peak alpha for the theme-derived top occlusion gradient.
    pub top_occlusion_peak_alpha: u32,
    /// Maximum visual-only boundary displacement for precise trackpad input.
    pub overscroll_max_distance: f32,
    /// Resistance distance used by the saturating rubber-band curve.
    pub overscroll_resistance: f32,
    /// Duration of the non-oscillating boundary rebound.
    pub overscroll_settle_ms: u64,
    /// Idle interval used when a platform stream omits an Ended phase.
    pub overscroll_idle_timeout_ms: u64,
    /// Reduced-motion cap; direct manipulation remains visible but minimal.
    pub overscroll_reduced_motion_max_distance: f32,
    pub section_padding_x: f32,
    pub section_padding_top: f32,
    pub section_padding_bottom: f32,
    pub section_gap: f32,
    pub section_icon_size: f32,
    pub main_hint_chip_padding_x: f32,
    pub main_hint_chip_padding_y: f32,
    pub main_hint_chip_radius: f32,
    pub main_hint_chip_font_size: f32,
    pub main_hint_chip_border_alpha: u32,
    pub main_hint_chip_bg_alpha: u32,
    pub main_hint_status_chip_gap: f32,
    pub main_hint_title_font_size: f32,
    pub main_hint_title_line_height: f32,
    pub main_hint_body_font_size: f32,
    pub main_hint_body_line_height: f32,
    pub main_hint_example_label_font_size: f32,
    pub main_hint_example_label_line_height: f32,
    pub main_hint_rows_gap: f32,
    pub main_hint_row_gap: f32,
    pub main_hint_row_label_width: f32,
    pub main_hint_row_label_font_size: f32,
    pub main_hint_row_label_line_height: f32,
    pub main_hint_row_label_alpha: u32,
    pub main_hint_row_value_font_size: f32,
    pub main_hint_row_value_line_height: f32,
    pub main_hint_row_value_alpha: u32,
    pub main_hint_fragment_rows_gap: f32,
    pub main_hint_fragment_row_gap: f32,
    pub main_hint_fragment_role_width: f32,
    pub main_hint_fragment_role_padding_x: f32,
    pub main_hint_fragment_role_padding_y: f32,
    pub main_hint_fragment_role_radius: f32,
    pub main_hint_fragment_role_font_size: f32,
    pub main_hint_fragment_role_line_height: f32,
    pub main_hint_fragment_role_border_alpha: u32,
    pub main_hint_fragment_role_bg_alpha: u32,
    pub main_hint_fragment_value_font_size: f32,
    pub main_hint_fragment_value_line_height: f32,
    pub main_hint_fragment_value_alpha: u32,
    pub main_hint_warning_border_alpha: u32,
    pub main_hint_warning_bg_alpha: u32,
    pub main_hint_divider_height: f32,
    pub main_hint_examples_group_gap: f32,
    pub main_hint_example_row_gap: f32,
    pub main_hint_form_focused_border_alpha: u32,
    pub main_hint_form_border_alpha: u32,
    pub main_hint_form_focused_bg_alpha: u32,
    pub main_hint_form_bg_alpha: u32,
    pub main_hint_form_label_alpha: u32,
    pub main_hint_form_value_alpha: u32,
    pub main_hint_form_label_font_size: f32,
    pub main_hint_form_label_line_height: f32,
    pub main_hint_form_input_font_size: f32,
    pub main_hint_form_input_line_height: f32,
    pub main_hint_form_value_font_size: f32,
    pub inline_calc_result_font_size: f32,
    pub inline_calc_hint_font_size: f32,
    pub inline_calc_selected_overlay_min_alpha: u32,
    pub inline_calc_selected_hint_alpha: u32,
    pub inline_calc_hint_alpha: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MainMenuRowTokens {
    pub outer_padding_x: f32,
    pub outer_padding_y: f32,
    pub inner_padding_x: f32,
    pub inner_padding_y: f32,
    pub radius: f32,
    pub name_desc_gap: f32,
    pub icon_text_gap: f32,
    pub accessory_gap: f32,
    pub selected_border_width: f32,
    pub selected_border_alpha: u32,
    pub selected_fill_alpha: u32,
    pub hover_fill_alpha: u32,
    pub selected_name_underline_width: f32,
    pub selected_name_underline_padding_bottom: f32,
    /// Width of the launcher-family selected-row marker overlay.
    pub selected_marker_width: f32,
    /// Height of the launcher-family selected-row marker overlay.
    pub selected_marker_height: f32,
    /// Horizontal inset from the selected row surface's left edge.
    pub selected_marker_inset_x: f32,
    /// Radius of the launcher-family selected-row marker overlay.
    pub selected_marker_radius: f32,
    /// Accent alpha of the launcher-family selected-row marker overlay.
    pub selected_marker_alpha: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MainMenuIconTokens {
    pub container_size: f32,
    pub svg_size: f32,
    pub tile_size: f32,
    pub tile_radius: f32,
    pub tile_fill_alpha: u32,
    pub tile_border_alpha: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MainMenuTypographyTokens {
    pub name_font_size: f32,
    pub name_line_height: f32,
    pub name_weight: FontWeight,
    pub selected_name_weight: FontWeight,
    pub desc_font_size: f32,
    pub desc_line_height: f32,
    pub desc_weight: FontWeight,
    pub section_font_size: f32,
    pub section_weight: FontWeight,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MainMenuMetadataTokens {
    pub metadata_alpha: u32,
    pub type_accessory_size: f32,
    pub source_font_size: f32,
    pub badge_font_size: f32,
    pub badge_padding_x: f32,
    pub badge_padding_y: f32,
    pub badge_radius: f32,
    pub keycap_font_size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FooterMetricsTokens {
    pub height_px: f32,
    pub side_inset_px: f32,
    pub item_gap_px: f32,
    pub content_gap: f32,
    pub button_padding_x: f32,
    pub button_padding_y: f32,
    pub run_button_padding_x: f32,
    pub button_radius: f32,
    pub label_font_size: f32,
    pub font_weight: FontWeight,
    pub keycap_padding_x: f32,
    pub keycap_padding_y: f32,
    /// Horizontal padding for multi-char word keycaps ("Space", "Enter");
    /// single glyphs keep the square min-width chip and `keycap_padding_x`.
    pub word_keycap_padding_x: f32,
    pub keycap_radius: f32,
    pub keycap_font_size: f32,
    pub keycap_height: f32,
    pub key_glyph_nudge_y: f32,
    pub return_glyph_nudge_y: f32,
    pub semicolon_glyph_nudge_y: f32,
    /// Optical corrections for the ⌘ glyph, whose ink sits low-left within
    /// its advance box at footer keycap sizes.
    pub cmd_glyph_nudge_x: f32,
    pub cmd_glyph_nudge_y: f32,
    /// Baseline correction for word keycaps: line-box centering leaves the
    /// cap+descender ink sitting low, so words ride slightly high of the
    /// single-glyph nudge.
    pub word_glyph_nudge_y: f32,
    pub run_slot_min_width: f32,
    pub run_slot_max_width: f32,
    pub actions_slot_width: f32,
    pub ai_slot_width: f32,
    pub apply_slot_width: f32,
    pub close_slot_width: f32,
    pub stop_slot_width: f32,
    pub paste_response_slot_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FooterTheme {
    pub divider_accent: bool,
    pub divider_alpha: u32,
    pub button: FooterButtonTheme,
    pub metrics: FooterMetricsTokens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeaderInfoBarLayout {
    Split,
    Breadcrumb,
    Compact,
    Plain,
    Centered,
    RightUtility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MainMenuLogoPlacement {
    InputLeading,
    HeaderLeading,
    HeaderTrailing,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MainMenuInputTextAlignment {
    RowTextColumn,
    SearchInset,
    SoftCenter,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeaderInfoBarTokens {
    pub layout: HeaderInfoBarLayout,
    pub font_family: &'static str,
    pub font_size: f32,
    pub opacity: f32,
    pub key_opacity: f32,
    pub height_px: f32,
    pub gap_px: f32,
    pub pill_padding_x: f32,
    pub pill_padding_y: f32,
    pub pill_radius: f32,
    pub pill_border_alpha: u32,
    pub pill_bg_alpha: u32,
    pub pill_hover_bg_alpha: u32,
    pub pill_hover_border_alpha: u32,
    pub pill_hover_text_alpha: u32,
    pub pill_hover_key_alpha: u32,
    pub context_edge_outset_x: f32,
    pub show_cwd: bool,
    pub show_agent_model: bool,
    pub show_keys: bool,
    pub separator: &'static str,
    pub logo_placement: MainMenuLogoPlacement,
    pub input_text_alignment: MainMenuInputTextAlignment,
    pub hide_initial_section_header: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MainMenuThemeDef {
    pub variant: MainMenuThemeVariant,
    pub name: &'static str,
    pub intent: &'static str,
    pub tier: MainMenuThemeTier,
    pub row_kind: MainMenuRowKind,
    pub shell: MainMenuShellTokens,
    pub search: MainMenuSearchTokens,
    pub list: MainMenuListTokens,
    pub row: MainMenuRowTokens,
    pub icon: MainMenuIconTokens,
    pub typography: MainMenuTypographyTokens,
    pub metadata: MainMenuMetadataTokens,
    pub footer: FooterTheme,
    pub header_info_bar: HeaderInfoBarTokens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MainMenuGeometrySignature {
    pub shell_inset_x: u32,
    pub header_padding_x: u32,
    pub search_height: u32,
    pub search_radius: u32,
    pub row_height: u32,
    pub row_padding_x: u32,
    pub row_radius: u32,
    pub selected_marker_width: u32,
    pub selected_marker_height: u32,
    pub selected_marker_inset_x: u32,
    pub selected_marker_radius: u32,
    pub selected_marker_alpha: u32,
    pub icon_container_size: u32,
    pub icon_tile_size: u32,
    pub name_font_size: u32,
    pub desc_font_size: u32,
    pub metadata_alpha: u32,
    pub footer_gap: u32,
    pub footer_button_radius: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HeaderInfoBarSignature {
    pub layout: HeaderInfoBarLayout,
    pub logo_placement: MainMenuLogoPlacement,
    pub input_text_alignment: MainMenuInputTextAlignment,
    pub font_size: u32,
    pub opacity: u32,
    pub key_opacity: u32,
    pub height_px: u32,
    pub gap_px: u32,
    pub pill_padding_x: u32,
    pub pill_padding_y: u32,
    pub pill_radius: u32,
    pub pill_border_alpha: u32,
    pub pill_bg_alpha: u32,
    pub pill_hover_bg_alpha: u32,
    pub pill_hover_border_alpha: u32,
    pub pill_hover_text_alpha: u32,
    pub pill_hover_key_alpha: u32,
    pub context_edge_outset_x: u32,
    pub show_cwd: bool,
    pub show_agent_model: bool,
    pub show_keys: bool,
    pub separator_len: usize,
    pub hide_initial_section_header: bool,
}

impl MainMenuThemeVariant {
    pub const COUNT: usize = 15;

    pub fn all() -> &'static [MainMenuThemeVariant] {
        use MainMenuThemeVariant::*;
        &[
            InfoBarBase,
            InfoBarBreadcrumb,
            InfoBarCompact,
            InfoBarSplit,
            InfoBarCwdFocus,
            InfoBarModelFocus,
            InfoBarSlashPath,
            InfoBarMutedPills,
            InfoBarPlainText,
            InfoBarLowContrastKeys,
            InfoBarStrongKeys,
            InfoBarCentered,
            InfoBarLeftDense,
            InfoBarRightUtility,
            InfoBarUltraQuiet,
        ]
    }

    pub fn from_u8(value: u8) -> MainMenuThemeVariant {
        Self::all()
            .iter()
            .copied()
            .find(|v| *v as u8 == value)
            .unwrap_or_default()
    }

    pub fn index(self) -> usize {
        Self::all().iter().position(|&v| v == self).unwrap_or(0)
    }

    pub fn name(self) -> &'static str {
        self.def().name
    }

    fn info_bar_name(self) -> &'static str {
        match self {
            MainMenuThemeVariant::InfoBarBase => "Base Info Bar",
            MainMenuThemeVariant::InfoBarBreadcrumb => "Right Model Breadcrumb",
            MainMenuThemeVariant::InfoBarCompact => "Right Model Compact",
            MainMenuThemeVariant::InfoBarSplit => "Right Model Split",
            MainMenuThemeVariant::InfoBarCwdFocus => "Right Model Cwd Focus",
            MainMenuThemeVariant::InfoBarModelFocus => "Right Model Focus",
            MainMenuThemeVariant::InfoBarSlashPath => "Right Model Slash Path",
            MainMenuThemeVariant::InfoBarMutedPills => "Right Model Muted Pills",
            MainMenuThemeVariant::InfoBarPlainText => "Right Model Plain",
            MainMenuThemeVariant::InfoBarLowContrastKeys => "Right Model Low Keys",
            MainMenuThemeVariant::InfoBarStrongKeys => "Right Model Strong Keys",
            MainMenuThemeVariant::InfoBarCentered => "Right Model Center Weight",
            MainMenuThemeVariant::InfoBarLeftDense => "Right Model Left Dense",
            MainMenuThemeVariant::InfoBarRightUtility => "Right Model Utility",
            MainMenuThemeVariant::InfoBarUltraQuiet => "Right Model Ultra Quiet",
        }
    }

    fn info_bar_intent(self) -> &'static str {
        match self {
            MainMenuThemeVariant::InfoBarBase => {
                "Smallest, dimmest right-model header using the current quiet layout as the base"
            }
            MainMenuThemeVariant::InfoBarBreadcrumb => {
                "Dimmest breadcrumb header with cwd left and model right"
            }
            MainMenuThemeVariant::InfoBarCompact => {
                "Dimmest compact header with minimal vertical footprint"
            }
            MainMenuThemeVariant::InfoBarSplit => "Dimmest split header with scan separation",
            MainMenuThemeVariant::InfoBarCwdFocus => "Dimmest cwd-weighted left chip",
            MainMenuThemeVariant::InfoBarModelFocus => "Dimmest model-weighted right chip",
            MainMenuThemeVariant::InfoBarSlashPath => "Dimmest slash-path rhythm",
            MainMenuThemeVariant::InfoBarMutedPills => "Dimmest muted pill boundaries",
            MainMenuThemeVariant::InfoBarPlainText => "Dimmest borderless split text",
            MainMenuThemeVariant::InfoBarLowContrastKeys => "Dimmest low-emphasis footer keycaps",
            MainMenuThemeVariant::InfoBarStrongKeys => "Dimmest clearer footer keycaps",
            MainMenuThemeVariant::InfoBarCentered => "Dimmest balanced split rhythm",
            MainMenuThemeVariant::InfoBarLeftDense => "Dimmest dense left chip",
            MainMenuThemeVariant::InfoBarRightUtility => "Dimmest right utility chip",
            MainMenuThemeVariant::InfoBarUltraQuiet => "Dimmest no-key metadata treatment",
        }
    }

    pub fn base_def(self) -> MainMenuThemeDef {
        base_main_menu_theme_def(self, self.header_info_bar())
    }

    pub fn def(self) -> MainMenuThemeDef {
        self.base_def()
    }

    pub fn header_info_bar(self) -> HeaderInfoBarTokens {
        use HeaderInfoBarLayout::*;
        match self {
            MainMenuThemeVariant::InfoBarBase => header_info_bar_tokens(
                Split,
                0.50,
                22.0,
                7.0,
                6.0,
                0.0,
                // Header context chips are hover-pill buttons: they share the
                // footer action buttons' radius and hovered keycap-border
                // alpha so every hover button in the app reads identically.
                MAIN_MENU_ACTION_BUTTON_RADIUS,
                0x00,
                0x00,
                0x10,
                MAIN_MENU_ACTION_BUTTON_HOVER_BORDER_ALPHA,
                0xff,
                0xff,
                true,
                true,
                true,
                "·",
            ),
            MainMenuThemeVariant::InfoBarBreadcrumb => header_info_bar_tokens(
                Split, 0.52, 14.0, 8.0, 4.0, 1.0, 4.0, 0x14, 0x00, 0x10, 0x34, 0xff, 0xff, true,
                true, false, "›",
            ),
            MainMenuThemeVariant::InfoBarCompact => header_info_bar_tokens(
                Split, 0.48, 13.0, 6.0, 0.0, 0.0, 0.0, 0x00, 0x00, 0x10, 0x34, 0xff, 0xff, true,
                true, false, "·",
            ),
            MainMenuThemeVariant::InfoBarSplit => header_info_bar_tokens(
                Split, 0.54, 15.0, 10.0, 5.0, 1.0, 5.0, 0x18, 0x08, 0x10, 0x34, 0xff, 0xff, true,
                true, false, "·",
            ),
            MainMenuThemeVariant::InfoBarCwdFocus => header_info_bar_tokens(
                Split, 0.46, 14.0, 7.0, 5.0, 1.0, 5.0, 0x1A, 0x00, 0x10, 0x34, 0xff, 0xff, true,
                true, false, "cwd",
            ),
            MainMenuThemeVariant::InfoBarModelFocus => header_info_bar_tokens(
                Split, 0.56, 15.0, 9.0, 4.0, 1.0, 5.0, 0x22, 0x0A, 0x10, 0x34, 0xff, 0xff, true,
                true, false, "model",
            ),
            MainMenuThemeVariant::InfoBarSlashPath => header_info_bar_tokens(
                Split, 0.50, 14.0, 6.0, 3.0, 0.0, 3.0, 0x00, 0x00, 0x10, 0x34, 0xff, 0xff, true,
                true, false, "/",
            ),
            MainMenuThemeVariant::InfoBarMutedPills => header_info_bar_tokens(
                Split, 0.44, 15.0, 8.0, 6.0, 1.0, 6.0, 0x10, 0x06, 0x10, 0x34, 0xff, 0xff, true,
                true, false, "·",
            ),
            MainMenuThemeVariant::InfoBarPlainText => header_info_bar_tokens(
                Split, 0.42, 13.0, 8.0, 0.0, 0.0, 0.0, 0x00, 0x00, 0x10, 0x34, 0xff, 0xff, true,
                true, false, "·",
            ),
            MainMenuThemeVariant::InfoBarLowContrastKeys => header_info_bar_tokens(
                Split, 0.38, 22.0, 9.0, 3.0, 1.0, 4.0, 0x12, 0x04, 0x10, 0x34, 0xff, 0xff, true,
                true, true, "·",
            ),
            MainMenuThemeVariant::InfoBarStrongKeys => header_info_bar_tokens(
                Split, 0.62, 22.0, 9.0, 4.0, 1.0, 5.0, 0x18, 0x08, 0x10, 0x34, 0xff, 0xff, true,
                true, true, "·",
            ),
            MainMenuThemeVariant::InfoBarCentered => header_info_bar_tokens(
                Split, 0.50, 14.0, 12.0, 2.0, 0.0, 2.0, 0x00, 0x00, 0x10, 0x34, 0xff, 0xff, true,
                true, false, "·",
            ),
            MainMenuThemeVariant::InfoBarLeftDense => header_info_bar_tokens(
                Split, 0.46, 13.0, 5.0, 3.0, 0.0, 3.0, 0x00, 0x00, 0x10, 0x34, 0xff, 0xff, true,
                true, false, "›",
            ),
            MainMenuThemeVariant::InfoBarRightUtility => header_info_bar_tokens(
                Split, 0.52, 16.0, 10.0, 6.0, 1.0, 6.0, 0x1C, 0x06, 0x10, 0x34, 0xff, 0xff, true,
                true, false, "·",
            ),
            MainMenuThemeVariant::InfoBarUltraQuiet => header_info_bar_tokens(
                Split, 0.34, 13.0, 7.0, 0.0, 0.0, 0.0, 0x00, 0x00, 0x10, 0x34, 0xff, 0xff, true,
                true, false, "·",
            ),
        }
    }

    pub fn header_info_bar_signature(self) -> HeaderInfoBarSignature {
        let tokens = self.header_info_bar();
        let q = |value: f32| (value * 100.0).round() as u32;
        HeaderInfoBarSignature {
            layout: tokens.layout,
            logo_placement: tokens.logo_placement,
            input_text_alignment: tokens.input_text_alignment,
            font_size: q(tokens.font_size),
            opacity: q(tokens.opacity),
            key_opacity: q(tokens.key_opacity),
            height_px: q(tokens.height_px),
            gap_px: q(tokens.gap_px),
            pill_padding_x: q(tokens.pill_padding_x),
            pill_padding_y: q(tokens.pill_padding_y),
            pill_radius: q(tokens.pill_radius),
            pill_border_alpha: tokens.pill_border_alpha,
            pill_bg_alpha: tokens.pill_bg_alpha,
            pill_hover_bg_alpha: tokens.pill_hover_bg_alpha,
            pill_hover_border_alpha: tokens.pill_hover_border_alpha,
            pill_hover_text_alpha: tokens.pill_hover_text_alpha,
            pill_hover_key_alpha: tokens.pill_hover_key_alpha,
            context_edge_outset_x: q(tokens.context_edge_outset_x),
            show_cwd: tokens.show_cwd,
            show_agent_model: tokens.show_agent_model,
            show_keys: tokens.show_keys,
            separator_len: tokens.separator.len(),
            hide_initial_section_header: tokens.hide_initial_section_header,
        }
    }

    pub fn geometry_signature(self) -> MainMenuGeometrySignature {
        let def = self.def();
        let q = |value: f32| (value * 10.0).round() as u32;
        MainMenuGeometrySignature {
            shell_inset_x: q(def.shell.content_inset_x),
            header_padding_x: q(def.shell.header_padding_x),
            search_height: q(def.search.height),
            search_radius: q(def.search.radius),
            row_height: q(def.list.item_height),
            row_padding_x: q(def.row.inner_padding_x),
            row_radius: q(def.row.radius),
            selected_marker_width: q(def.row.selected_marker_width),
            selected_marker_height: q(def.row.selected_marker_height),
            selected_marker_inset_x: q(def.row.selected_marker_inset_x),
            selected_marker_radius: q(def.row.selected_marker_radius),
            selected_marker_alpha: def.row.selected_marker_alpha,
            icon_container_size: q(def.icon.container_size),
            icon_tile_size: q(def.icon.tile_size),
            name_font_size: q(def.typography.name_font_size),
            desc_font_size: q(def.typography.desc_font_size),
            metadata_alpha: def.metadata.metadata_alpha,
            footer_gap: q(def.footer.metrics.item_gap_px),
            footer_button_radius: q(def.footer.metrics.button_radius),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn header_info_bar_tokens(
    layout: HeaderInfoBarLayout,
    key_opacity: f32,
    height_px: f32,
    gap_px: f32,
    pill_padding_x: f32,
    pill_padding_y: f32,
    pill_radius: f32,
    pill_border_alpha: u32,
    pill_bg_alpha: u32,
    pill_hover_bg_alpha: u32,
    pill_hover_border_alpha: u32,
    pill_hover_text_alpha: u32,
    pill_hover_key_alpha: u32,
    show_cwd: bool,
    show_agent_model: bool,
    show_keys: bool,
    separator: &'static str,
) -> HeaderInfoBarTokens {
    HeaderInfoBarTokens {
        layout,
        font_family: "JetBrains Mono",
        font_size: 10.5,
        opacity: 0.34,
        key_opacity,
        height_px,
        gap_px,
        pill_padding_x,
        pill_padding_y,
        pill_radius,
        pill_border_alpha,
        pill_bg_alpha,
        pill_hover_bg_alpha,
        pill_hover_border_alpha,
        pill_hover_text_alpha,
        pill_hover_key_alpha,
        context_edge_outset_x: MAIN_MENU_HEADER_CONTEXT_EDGE_OUTSET_X,
        show_cwd,
        show_agent_model,
        show_keys,
        separator,
        logo_placement: MainMenuLogoPlacement::Hidden,
        input_text_alignment: MainMenuInputTextAlignment::RowTextColumn,
        hide_initial_section_header: false,
    }
}

fn main_menu_row_tokens(row_kind: MainMenuRowKind) -> MainMenuRowTokens {
    let mut row = MainMenuRowTokens {
        outer_padding_x: 4.0,
        outer_padding_y: 0.0,
        inner_padding_x: 14.0,
        inner_padding_y: 4.0,
        radius: 14.0,
        name_desc_gap: (40.0_f32 / 18.0).clamp(1.5, 3.5),
        icon_text_gap: (20.0_f32 * 0.42).clamp(7.0, 12.0),
        accessory_gap: (14.0_f32 * 0.45).clamp(5.0, 9.0),
        selected_border_width: 0.0,
        selected_border_alpha: 0x00,
        selected_fill_alpha: 0x20,
        hover_fill_alpha: 0x12,
        selected_name_underline_width: 0.0,
        selected_name_underline_padding_bottom: 0.0,
        selected_marker_width: 2.0,
        selected_marker_height: 16.0,
        selected_marker_inset_x: 6.0,
        selected_marker_radius: 1.0,
        selected_marker_alpha: 0xFF,
    };

    if matches!(row_kind, MainMenuRowKind::CarbonNeon) {
        row.selected_name_underline_width = 2.0;
        row.selected_name_underline_padding_bottom = 1.0;
    }

    row
}

fn base_main_menu_theme_def(
    variant: MainMenuThemeVariant,
    header_info_bar: HeaderInfoBarTokens,
) -> MainMenuThemeDef {
    let row_kind = MainMenuRowKind::IconTile;
    let footer = FooterTheme {
        divider_accent: false,
        divider_alpha: 20,
        button: FooterButtonTheme {
            border_alpha: 50,
            hover_border_alpha: MAIN_MENU_ACTION_BUTTON_HOVER_BORDER_ALPHA,
        },
        metrics: FooterMetricsTokens {
            height_px: 32.0,
            side_inset_px: 14.0,
            item_gap_px: 2.0,
            content_gap: 4.0,
            button_padding_x: 4.0,
            button_padding_y: 2.0,
            run_button_padding_x: 12.0,
            button_radius: MAIN_MENU_ACTION_BUTTON_RADIUS,
            label_font_size: 13.0,
            font_weight: FontWeight(400.0),
            keycap_padding_x: 0.0,
            keycap_padding_y: 0.0,
            word_keycap_padding_x: 5.0,
            keycap_radius: 6.0,
            keycap_font_size: 11.0,
            keycap_height: 20.0,
            key_glyph_nudge_y: 1.0,
            return_glyph_nudge_y: 1.0,
            semicolon_glyph_nudge_y: -1.0,
            cmd_glyph_nudge_x: 0.5,
            cmd_glyph_nudge_y: 0.0,
            word_glyph_nudge_y: -0.75,
            run_slot_min_width: 92.0,
            run_slot_max_width: 242.0,
            actions_slot_width: 92.0,
            ai_slot_width: 52.0,
            apply_slot_width: 84.0,
            close_slot_width: 84.0,
            stop_slot_width: 76.0,
            paste_response_slot_width: 140.0,
        },
    };

    MainMenuThemeDef {
        variant,
        name: variant.info_bar_name(),
        intent: variant.info_bar_intent(),
        tier: MainMenuThemeTier::TahoeClose,
        row_kind,
        shell: MainMenuShellTokens {
            content_inset_x: 16.0,
            content_inset_bottom: 16.0 * 0.35,
            header_padding_x: 2.0,
            header_padding_y: 4.0,
            header_gap: 2.0,
            divider_margin_x: 16.0,
            divider_height: 0.0,
            divider_alpha: footer.divider_alpha,
        },
        search: MainMenuSearchTokens {
            height: 26.0,
            radius: 9.0,
            text_inset_x: crate::ui::chrome::SEARCH_INPUT_TEXT_INSET_X_PX,
            text_inset_y: 0.0,
            surface_alpha: 0x00,
            border_alpha: 0x00,
            font_size: 20.0,
            font_weight: FontWeight(430.0),
        },
        list: MainMenuListTokens {
            item_height: 44.0,
            section_header_height: 28.0,
            first_section_header_height: 28.0,
            source_status_row_height: 32.0,
            average_scroll_height: ((44.0 * 3.0) + 28.0) / 4.0,
            footer_reveal_clearance_height: 0.0,
            scrollbar_width: 16.0,
            top_occlusion_height: 10.0,
            top_occlusion_ramp: 96.0,
            top_occlusion_peak_alpha: 0x2E,
            overscroll_max_distance: 18.0,
            overscroll_resistance: 36.0,
            overscroll_settle_ms: 180,
            overscroll_idle_timeout_ms: 160,
            overscroll_reduced_motion_max_distance: 4.0,
            section_padding_x: MAIN_MENU_SECTION_PADDING_X,
            section_padding_top: MAIN_MENU_SECTION_PADDING_TOP,
            section_padding_bottom: MAIN_MENU_SECTION_PADDING_BOTTOM,
            section_gap: MAIN_MENU_SECTION_GAP,
            section_icon_size: MAIN_MENU_SECTION_ICON_SIZE,
            main_hint_chip_padding_x: 8.0,
            main_hint_chip_padding_y: 3.0,
            main_hint_chip_radius: 6.0,
            main_hint_chip_font_size: 11.0,
            main_hint_chip_border_alpha: 0x66,
            main_hint_chip_bg_alpha: 0x18,
            main_hint_status_chip_gap: 6.0,
            main_hint_title_font_size: 18.0,
            main_hint_title_line_height: 24.0,
            main_hint_body_font_size: 13.0,
            main_hint_body_line_height: 18.0,
            main_hint_example_label_font_size: 12.0,
            main_hint_example_label_line_height: 16.0,
            main_hint_rows_gap: 7.0,
            main_hint_row_gap: 12.0,
            main_hint_row_label_width: 76.0,
            main_hint_row_label_font_size: 12.0,
            main_hint_row_label_line_height: 18.0,
            main_hint_row_label_alpha: 0xCC,
            main_hint_row_value_font_size: 13.0,
            main_hint_row_value_line_height: 18.0,
            main_hint_row_value_alpha: 0xE6,
            main_hint_fragment_rows_gap: 6.0,
            main_hint_fragment_row_gap: 10.0,
            main_hint_fragment_role_width: 82.0,
            main_hint_fragment_role_padding_x: 7.0,
            main_hint_fragment_role_padding_y: 2.0,
            main_hint_fragment_role_radius: 5.0,
            main_hint_fragment_role_font_size: 10.0,
            main_hint_fragment_role_line_height: 14.0,
            main_hint_fragment_role_border_alpha: 0x55,
            main_hint_fragment_role_bg_alpha: 0x14,
            main_hint_fragment_value_font_size: 12.0,
            main_hint_fragment_value_line_height: 17.0,
            main_hint_fragment_value_alpha: 0xE6,
            main_hint_warning_border_alpha: 0x66,
            main_hint_warning_bg_alpha: 0x14,
            main_hint_divider_height: 1.0,
            main_hint_examples_group_gap: 5.0,
            main_hint_example_row_gap: 3.0,
            main_hint_form_focused_border_alpha: 0xF2,
            main_hint_form_border_alpha: 0x80,
            main_hint_form_focused_bg_alpha: 0x3D,
            main_hint_form_bg_alpha: 0x24,
            main_hint_form_label_alpha: 0xB3,
            main_hint_form_value_alpha: 0xFF,
            main_hint_form_label_font_size: 12.0,
            main_hint_form_label_line_height: 18.0,
            main_hint_form_input_font_size: 16.0,
            main_hint_form_input_line_height: 24.0,
            main_hint_form_value_font_size: 14.0,
            inline_calc_result_font_size: 16.0,
            inline_calc_hint_font_size: 10.0,
            inline_calc_selected_overlay_min_alpha: 0x2E,
            inline_calc_selected_hint_alpha: 0xD9,
            inline_calc_hint_alpha: 0x8C,
        },
        row: main_menu_row_tokens(row_kind),
        icon: MainMenuIconTokens {
            container_size: 20.0,
            svg_size: 16.0,
            tile_size: 20.0,
            tile_radius: 7.0,
            tile_fill_alpha: 0x80,
            tile_border_alpha: 0x00,
        },
        typography: MainMenuTypographyTokens {
            name_font_size: 14.0,
            name_line_height: 16.0,
            name_weight: FontWeight(450.0),
            selected_name_weight: FontWeight::MEDIUM,
            desc_font_size: 12.0,
            desc_line_height: 16.0,
            desc_weight: FontWeight::NORMAL,
            section_font_size: 12.0,
            section_weight: MAIN_MENU_SECTION_WEIGHT,
        },
        metadata: MainMenuMetadataTokens {
            metadata_alpha: 0x73,
            type_accessory_size: 12.0,
            source_font_size: MAIN_MENU_METADATA_SOURCE_FONT_SIZE,
            badge_font_size: MAIN_MENU_METADATA_BADGE_FONT_SIZE,
            badge_padding_x: MAIN_MENU_METADATA_BADGE_PADDING_X,
            badge_padding_y: MAIN_MENU_METADATA_BADGE_PADDING_Y,
            badge_radius: MAIN_MENU_METADATA_BADGE_RADIUS,
            keycap_font_size: footer.metrics.keycap_font_size,
        },
        footer,
        header_info_bar,
    }
}

static CURRENT_MAIN_MENU_THEME: AtomicU8 = AtomicU8::new(MainMenuThemeVariant::InfoBarBase as u8);

pub fn set_current_main_menu_theme(theme: MainMenuThemeVariant) {
    CURRENT_MAIN_MENU_THEME.store(theme as u8, Ordering::Relaxed);
}

pub fn current_main_menu_theme() -> MainMenuThemeVariant {
    MainMenuThemeVariant::from_u8(CURRENT_MAIN_MENU_THEME.load(Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::{current_main_menu_theme, set_current_main_menu_theme, MainMenuThemeVariant};
    use std::collections::HashSet;

    #[test]
    fn header_info_bar_has_exactly_fifteen_variants() {
        assert_eq!(
            MainMenuThemeVariant::all().len(),
            MainMenuThemeVariant::COUNT
        );
        assert_eq!(MainMenuThemeVariant::COUNT, 15);
        for v in MainMenuThemeVariant::all() {
            assert_eq!(MainMenuThemeVariant::from_u8(*v as u8), *v);
        }
    }

    #[test]
    fn header_names_are_identifiable() {
        for theme in MainMenuThemeVariant::all() {
            assert!(!theme.name().trim().is_empty());
        }
    }

    #[test]
    fn footer_button_theme_only_owns_keycap_border_policy() {
        for theme in MainMenuThemeVariant::all() {
            let button = theme.def().footer.button;
            assert!(
                button.border_alpha <= button.hover_border_alpha,
                "{theme:?} hover keycap border must not be weaker than rest"
            );
        }
    }

    #[test]
    fn default_is_base_info_bar_and_first() {
        assert_eq!(
            MainMenuThemeVariant::default(),
            MainMenuThemeVariant::InfoBarBase
        );
        assert_eq!(MainMenuThemeVariant::default().index(), 0);
    }

    #[test]
    fn global_round_trips_through_u8() {
        for v in MainMenuThemeVariant::all() {
            set_current_main_menu_theme(*v);
            assert_eq!(current_main_menu_theme(), *v);
        }
        set_current_main_menu_theme(MainMenuThemeVariant::default());
    }

    #[test]
    fn every_variant_preserves_base_non_header_geometry() {
        let base = MainMenuThemeVariant::InfoBarBase.geometry_signature();
        for theme in MainMenuThemeVariant::all() {
            assert_eq!(
                theme.geometry_signature(),
                base,
                "{theme:?} changed base geometry"
            );
        }
    }

    #[test]
    fn every_variant_uses_the_approved_launcher_selection_marker() {
        for theme in MainMenuThemeVariant::all() {
            let marker = theme.def().row;
            assert_eq!(marker.selected_marker_width, 2.0, "{theme:?} width");
            assert_eq!(marker.selected_marker_height, 16.0, "{theme:?} height");
            assert_eq!(marker.selected_marker_inset_x, 6.0, "{theme:?} inset");
            assert_eq!(marker.selected_marker_radius, 1.0, "{theme:?} radius");
            assert_eq!(marker.selected_marker_alpha, 0xFF, "{theme:?} alpha");
        }
    }

    #[test]
    fn every_variant_has_distinct_header_info_bar_signature() {
        let mut signatures = HashSet::new();
        for theme in MainMenuThemeVariant::all() {
            assert!(
                signatures.insert(theme.header_info_bar_signature()),
                "{theme:?} reused a header info-bar signature"
            );
        }
    }

    #[test]
    fn header_info_bar_uses_mono_and_reduced_opacity() {
        for theme in MainMenuThemeVariant::all() {
            let tokens = theme.def().header_info_bar;
            assert_eq!(tokens.font_family, "JetBrains Mono");
            assert_eq!(tokens.font_size, 10.5);
            assert_eq!(tokens.opacity, 0.34);
            assert!(tokens.height_px <= 22.0);
            assert!(tokens.show_cwd);
            assert!(tokens.show_agent_model);
        }
    }
}

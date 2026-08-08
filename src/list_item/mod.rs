//! Shared ListItem component for script list and arg prompt choice list
//!
//! This module provides a reusable, theme-aware list item component that can be
//! used in both the main script list and arg prompt choice lists.

#![allow(dead_code)]

/// GEO-001 canonical geometry-role vocabulary (roles, relations, comparability).
pub mod geometry_roles;
/// GEO-009 explicit list presentation modes and predictive metric resolvers.
pub mod metrics;

use crate::designs::icon_variations::{icon_name_from_str, IconName};
use crate::logging;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::tooltip::Tooltip;
use std::sync::Arc;

/// Temporary product policy: list-row hover must stay quiet while control
/// tooltips elsewhere in the UI remain enabled.
pub(crate) const LIST_ITEM_MOUSE_HOVER_TOOLTIPS_ENABLED: bool = false;
/// Icon type for list items - supports emoji strings, SVG icons, and pre-decoded images
#[derive(Clone)]
pub enum IconKind {
    /// Text/emoji icon (e.g., "📜", "⚡")
    Emoji(String),
    /// Pre-decoded render image (for app icons) - MUST be pre-decoded, not raw PNG bytes
    Image(Arc<RenderImage>),
    /// SVG icon by name (e.g., "File", "Terminal", "Code")
    /// Maps to IconName from designs::icon_variations
    Svg(String),
}

impl IconKind {
    /// Resolve icon metadata into an IconKind for list rendering.
    ///
    /// Supports:
    /// - Known SVG names/aliases via `icon_name_from_str` (e.g., "Terminal", "file-code")
    /// - Emoji/symbol glyphs (e.g., "📄", "⚡")
    pub fn from_icon_hint(icon_hint: &str) -> Option<Self> {
        let trimmed = icon_hint.trim();
        if trimmed.is_empty() {
            return None;
        }

        if icon_name_from_str(trimmed).is_some() {
            return Some(Self::Svg(trimmed.to_string()));
        }

        if looks_like_symbol_icon_hint(trimmed) {
            return Some(Self::Emoji(trimmed.to_string()));
        }

        None
    }
}

fn looks_like_symbol_icon_hint(icon_hint: &str) -> bool {
    let has_ascii_alnum = icon_hint.chars().any(|ch| ch.is_ascii_alphanumeric());
    let char_count = icon_hint.chars().count();

    !has_ascii_alnum && char_count <= 4
}

/// Resolve an icon hint to an SVG source for list-item rendering.
///
/// Returns either an embedded asset path ("icons/foo.svg", served by
/// `AppAssets`) or an absolute on-disk path for script-provided icon files.
/// Render with `crate::utils::assets::svg_icon_source`, which picks the
/// loader by path kind. Never bake CARGO_MANIFEST_DIR filesystem paths here:
/// they point at the CI runner in released bundles (P0 2026-06-11).
fn resolve_svg_icon_path(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.ends_with(".svg") {
        let explicit_path = std::path::Path::new(trimmed);
        if explicit_path.is_absolute() && explicit_path.exists() {
            return trimmed.to_string();
        }
        if crate::utils::assets::embedded_asset_exists(trimmed) {
            return trimmed.to_string();
        }
    }

    if let Some(icon_name) = icon_name_from_str(trimmed) {
        icon_name.asset_path().to_string()
    } else {
        let lucide_path = format!("icons/{trimmed}.svg");
        if crate::utils::assets::embedded_asset_exists(&lucide_path) {
            lucide_path
        } else {
            IconName::Code.asset_path().to_string()
        }
    }
}

/// Fixed height for list items used in uniform-height virtualized lists.
///
/// IMPORTANT: When using GPUI `uniform_list`, the item closure must render
/// at exactly this height (including padding). If you change visuals, keep the
/// total height stable or update this constant everywhere it is used.
pub const LIST_ITEM_HEIGHT: f32 = 40.0;
/// Fixed height for section headers (RECENT, MAIN, etc.)
/// Total height includes: pt(12px) + text (~12px) + pb(4px) = ~28px content
/// Using 32px (8px grid) for comfortable spacing while maintaining visual compactness.
///
/// ## Performance Note (uniform_list vs list)
/// - Use `uniform_list` when every row has the same fixed height (fast O(1) scroll math).
/// - Use `list()` when you need variable heights (e.g., headers + items); it uses a SumTree
///   and scroll math is O(log n).
pub const SECTION_HEADER_HEIGHT: f32 = 32.0;
/// Fixed height for transient source status rows before they are split out of the ScriptList row model.
pub const SOURCE_STATUS_ROW_HEIGHT: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListItemMetricsOverride {
    pub item_height: f32,
    pub section_header_height: f32,
    pub first_section_header_height: f32,
    pub source_status_row_height: f32,
    pub average_scroll_height: f32,
    pub section_padding_x: f32,
    pub section_padding_top: f32,
    pub first_section_padding_top: f32,
    pub section_padding_bottom: f32,
    pub icon_container_size: f32,
    pub icon_svg_size: f32,
    pub icon_tile_size: f32,
    pub icon_tile_radius: f32,
    pub icon_tile_fill_alpha: u32,
    pub icon_tile_border_alpha: u32,
    pub row_outer_padding_x: f32,
    pub row_outer_padding_y: f32,
    pub row_inner_padding_x: f32,
    pub row_inner_padding_y: f32,
    pub row_radius: f32,
    pub row_selected_border_width: f32,
    pub row_selected_border_alpha: u32,
    pub row_selected_fill_alpha: u32,
    pub row_hover_fill_alpha: u32,
    pub row_selected_name_underline_width: f32,
    pub row_selected_name_underline_padding_bottom: f32,
    pub row_selected_marker_width: f32,
    pub row_selected_marker_height: f32,
    pub row_selected_marker_inset_x: f32,
    pub row_selected_marker_radius: f32,
    pub row_selected_marker_alpha: u32,
    /// Host row height used only to vertically center the absolute marker.
    /// Compact launcher-family hosts may override this without changing ListItem content geometry.
    pub row_selected_marker_center_height: f32,
    pub icon_text_gap: f32,
    pub name_desc_gap: f32,
    pub accessory_gap: f32,
    pub name_font_size: f32,
    pub name_line_height: f32,
    pub desc_font_size: f32,
    pub desc_line_height: f32,
    pub source_font_size: f32,
    pub badge_font_size: f32,
    pub badge_padding_x: f32,
    pub badge_padding_y: f32,
    pub badge_radius: f32,
    pub section_header_font_size: f32,
    pub section_gap: f32,
    pub section_icon_size: f32,
    pub name_weight: FontWeight,
    pub selected_name_weight: FontWeight,
    pub desc_weight: FontWeight,
    pub section_weight: FontWeight,
    pub desc_quiet_alpha: u32,
}

impl ListItemMetricsOverride {
    pub const fn default_main_menu() -> Self {
        Self {
            item_height: LIST_ITEM_HEIGHT,
            section_header_height: SECTION_HEADER_HEIGHT,
            first_section_header_height: SECTION_HEADER_HEIGHT
                - (crate::designs::MAIN_MENU_SECTION_PADDING_TOP / 2.0),
            source_status_row_height: SOURCE_STATUS_ROW_HEIGHT,
            average_scroll_height: AVERAGE_ITEM_HEIGHT_FOR_SCROLL,
            section_padding_x: crate::designs::MAIN_MENU_SECTION_PADDING_X,
            section_padding_top: crate::designs::MAIN_MENU_SECTION_PADDING_TOP,
            first_section_padding_top: crate::designs::MAIN_MENU_SECTION_PADDING_TOP / 2.0,
            section_padding_bottom: crate::designs::MAIN_MENU_SECTION_PADDING_BOTTOM,
            icon_container_size: ICON_CONTAINER_SIZE,
            icon_svg_size: ICON_SVG_SIZE,
            icon_tile_size: ICON_CONTAINER_SIZE,
            icon_tile_radius: 7.0,
            icon_tile_fill_alpha: 0xE6,
            icon_tile_border_alpha: 0x00,
            row_outer_padding_x: 4.0,
            row_outer_padding_y: 2.0,
            row_inner_padding_x: ITEM_PADDING_X,
            row_inner_padding_y: ITEM_PADDING_Y,
            row_radius: LIST_ITEM_ROW_RADIUS_PX,
            row_selected_border_width: 0.0,
            row_selected_border_alpha: 0x00,
            row_selected_fill_alpha: 0x20,
            row_hover_fill_alpha: 0x12,
            row_selected_name_underline_width: 0.0,
            row_selected_name_underline_padding_bottom: 0.0,
            row_selected_marker_width: 2.0,
            row_selected_marker_height: 16.0,
            row_selected_marker_inset_x: 6.0,
            row_selected_marker_radius: 1.0,
            row_selected_marker_alpha: 0xFF,
            row_selected_marker_center_height: LIST_ITEM_HEIGHT,
            icon_text_gap: ITEM_ICON_TEXT_GAP,
            name_desc_gap: ITEM_NAME_DESC_GAP,
            accessory_gap: ITEM_ACCESSORIES_GAP,
            name_font_size: NAME_FONT_SIZE,
            name_line_height: NAME_LINE_HEIGHT,
            desc_font_size: DESC_FONT_SIZE,
            desc_line_height: DESC_LINE_HEIGHT,
            source_font_size: crate::designs::MAIN_MENU_METADATA_SOURCE_FONT_SIZE,
            badge_font_size: crate::designs::MAIN_MENU_METADATA_BADGE_FONT_SIZE,
            badge_padding_x: crate::designs::MAIN_MENU_METADATA_BADGE_PADDING_X,
            badge_padding_y: crate::designs::MAIN_MENU_METADATA_BADGE_PADDING_Y,
            badge_radius: crate::designs::MAIN_MENU_METADATA_BADGE_RADIUS,
            section_header_font_size: SECTION_HEADER_FONT_SIZE,
            section_gap: crate::designs::MAIN_MENU_SECTION_GAP,
            section_icon_size: crate::designs::MAIN_MENU_SECTION_ICON_SIZE,
            name_weight: FontWeight(450.0),
            selected_name_weight: FontWeight::MEDIUM,
            desc_weight: FontWeight::NORMAL,
            section_weight: crate::designs::MAIN_MENU_SECTION_WEIGHT,
            desc_quiet_alpha: ALPHA_DESC_QUIET,
        }
    }

    pub fn from_main_menu_theme(theme: crate::designs::MainMenuThemeVariant) -> Self {
        Self::from_main_menu_def(theme.def())
    }

    /// Metrics from an explicit main-menu def. The design-contract exporter
    /// passes `base_def()` here so checked-in artifacts always match the base
    /// token definition; production rendering keeps using
    /// [`Self::from_main_menu_theme`].
    pub fn from_main_menu_def(def: crate::designs::MainMenuThemeDef) -> Self {
        Self {
            item_height: def.list.item_height,
            section_header_height: def.list.section_header_height,
            first_section_header_height: def.list.first_section_header_height,
            source_status_row_height: def.list.source_status_row_height,
            average_scroll_height: def.list.average_scroll_height,
            section_padding_x: def.list.section_padding_x,
            section_padding_top: def.list.section_padding_top,
            first_section_padding_top: def.shell.header_padding_y,
            section_padding_bottom: def.list.section_padding_bottom,
            icon_container_size: def.icon.container_size,
            icon_svg_size: def.icon.svg_size,
            icon_tile_size: def.icon.tile_size,
            icon_tile_radius: def.icon.tile_radius,
            icon_tile_fill_alpha: def.icon.tile_fill_alpha,
            icon_tile_border_alpha: def.icon.tile_border_alpha,
            row_outer_padding_x: def.row.outer_padding_x,
            row_outer_padding_y: def.row.outer_padding_y,
            row_inner_padding_x: def.row.inner_padding_x,
            row_inner_padding_y: def.row.inner_padding_y,
            row_radius: def.row.radius,
            row_selected_border_width: def.row.selected_border_width,
            row_selected_border_alpha: def.row.selected_border_alpha,
            row_selected_fill_alpha: def.row.selected_fill_alpha,
            row_hover_fill_alpha: def.row.hover_fill_alpha,
            row_selected_name_underline_width: def.row.selected_name_underline_width,
            row_selected_name_underline_padding_bottom: def
                .row
                .selected_name_underline_padding_bottom,
            row_selected_marker_width: def.row.selected_marker_width,
            row_selected_marker_height: def.row.selected_marker_height,
            row_selected_marker_inset_x: def.row.selected_marker_inset_x,
            row_selected_marker_radius: def.row.selected_marker_radius,
            row_selected_marker_alpha: def.row.selected_marker_alpha,
            row_selected_marker_center_height: def.list.item_height,
            icon_text_gap: def.row.icon_text_gap,
            name_desc_gap: def.row.name_desc_gap,
            accessory_gap: def.row.accessory_gap,
            name_font_size: def.typography.name_font_size,
            name_line_height: def.typography.name_line_height,
            desc_font_size: def.typography.desc_font_size,
            desc_line_height: def.typography.desc_line_height,
            source_font_size: def.metadata.source_font_size,
            badge_font_size: def.metadata.badge_font_size,
            badge_padding_x: def.metadata.badge_padding_x,
            badge_padding_y: def.metadata.badge_padding_y,
            badge_radius: def.metadata.badge_radius,
            section_header_font_size: def.typography.section_font_size,
            section_gap: def.list.section_gap,
            section_icon_size: def.list.section_icon_size,
            name_weight: def.typography.name_weight,
            selected_name_weight: def.typography.selected_name_weight,
            desc_weight: def.typography.desc_weight,
            section_weight: def.typography.section_weight,
            desc_quiet_alpha: def.metadata.metadata_alpha,
        }
    }
}

/// Geometry for the launcher-family selected-row marker overlay.
///
/// These coordinates are relative to the outer row container. The marker is
/// deliberately independent from flex layout so it cannot move row content,
/// alter hit bounds, or participate in scroll measurement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListItemSelectionMarkerGeometry {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub radius: f32,
    pub alpha: u32,
}

pub fn list_item_selection_marker_geometry(
    metrics: ListItemMetricsOverride,
    selected: bool,
) -> Option<ListItemSelectionMarkerGeometry> {
    selected.then(|| ListItemSelectionMarkerGeometry {
        left: metrics.row_outer_padding_x + metrics.row_selected_marker_inset_x,
        top: (metrics.row_selected_marker_center_height - metrics.row_selected_marker_height) / 2.0,
        width: metrics.row_selected_marker_width,
        height: metrics.row_selected_marker_height,
        radius: metrics.row_selected_marker_radius,
        alpha: metrics.row_selected_marker_alpha,
    })
}

pub use crate::theme::MainMenuRowFillBase;

/// Fill bytes a main-menu row actually paints. Shared by `ListItem::render`
/// and the design-contract exporter (`design/mockups`) so the HTML mockups
/// consume the exact alphas the renderer uses — do not duplicate this
/// row-kind match anywhere else.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedMainMenuRowFill {
    pub base: MainMenuRowFillBase,
    pub selected_alpha: u8,
    pub hover_alpha: u8,
    pub icon_tile_alpha: u32,
    pub icon_tile_radius: f32,
}

pub fn resolved_main_menu_row_fill(
    row_kind: crate::designs::MainMenuRowKind,
    metrics: &ListItemMetricsOverride,
    theme_hover_opacity: f32,
) -> ResolvedMainMenuRowFill {
    use crate::designs::MainMenuRowKind as K;
    let fill = crate::theme::resolve_main_menu_row_state_fill(
        row_kind,
        metrics.row_hover_fill_alpha as u8,
        metrics.row_selected_fill_alpha as u8,
        theme_hover_opacity,
    );
    let icon_tile_alpha_floor: u32 = match row_kind {
        K::MilkGlass => 0xB8,
        K::ProConsole => 0xC8,
        K::LiquidPrism | K::StudioPaperGlass => 0xCC,
        K::FrostedCommand => 0xD0,
        K::AuroraSlate | K::OceanGlass => 0xD8,
        K::SpotlightLuxe => 0xE0,
        K::WarmGold | K::BlueGlass => 0xE6,
        K::CarbonNeon => 0xF0,
        _ => 0xF2,
    };
    let icon_tile_radius_floor: f32 = match row_kind {
        K::ProConsole | K::CarbonNeon | K::OperatorMonoGlass => 6.0,
        K::SpotlightLuxe => 10.0,
        _ => 7.0,
    };
    ResolvedMainMenuRowFill {
        base: fill.base,
        selected_alpha: fill.active_alpha,
        hover_alpha: fill.hover_alpha,
        icon_tile_alpha: metrics.icon_tile_fill_alpha.max(icon_tile_alpha_floor),
        icon_tile_radius: metrics.icon_tile_radius.max(icon_tile_radius_floor),
    }
}

#[inline]
fn resolved_list_item_metrics() -> ListItemMetricsOverride {
    #[cfg(feature = "storybook")]
    if let Some(metrics) = crate::storybook::adopted_main_menu_list_study_metrics() {
        return metrics;
    }

    ListItemMetricsOverride::default_main_menu()
}

#[inline]
pub fn effective_list_item_height() -> f32 {
    resolved_list_item_metrics().item_height
}

#[inline]
pub fn effective_list_item_height_for_theme(theme: crate::designs::MainMenuThemeVariant) -> f32 {
    ListItemMetricsOverride::from_main_menu_theme(theme).item_height
}

#[inline]
pub fn effective_section_header_height() -> f32 {
    resolved_list_item_metrics().section_header_height
}

#[inline]
pub fn effective_section_header_height_for_theme(
    theme: crate::designs::MainMenuThemeVariant,
) -> f32 {
    ListItemMetricsOverride::from_main_menu_theme(theme).section_header_height
}

#[inline]
pub fn effective_first_section_header_height() -> f32 {
    let metrics = resolved_list_item_metrics();
    metrics.section_header_height - (metrics.section_padding_top / 2.0)
}

#[inline]
pub fn effective_first_section_header_height_for_theme(
    theme: crate::designs::MainMenuThemeVariant,
) -> f32 {
    ListItemMetricsOverride::from_main_menu_theme(theme).first_section_header_height
}

#[inline]
pub fn effective_source_status_row_height() -> f32 {
    effective_source_status_row_height_for_theme(crate::designs::current_main_menu_theme())
}

#[inline]
pub fn effective_source_status_row_height_for_theme(
    theme: crate::designs::MainMenuThemeVariant,
) -> f32 {
    ListItemMetricsOverride::from_main_menu_theme(theme).source_status_row_height
}

#[inline]
pub fn effective_average_item_height_for_scroll() -> f32 {
    let metrics = resolved_list_item_metrics();
    ((metrics.item_height * 3.0) + metrics.section_header_height) / 4.0
}

#[inline]
pub fn effective_average_item_height_for_scroll_for_theme(
    theme: crate::designs::MainMenuThemeVariant,
) -> f32 {
    ListItemMetricsOverride::from_main_menu_theme(theme).average_scroll_height
}

#[inline]
pub fn effective_footer_reveal_clearance_height() -> f32 {
    effective_footer_reveal_clearance_height_for_theme(crate::designs::current_main_menu_theme())
}

#[inline]
pub fn effective_footer_reveal_clearance_height_for_theme(
    theme: crate::designs::MainMenuThemeVariant,
) -> f32 {
    theme.def().list.footer_reveal_clearance_height
}
// =============================================================================
// Layout & Spacing Constants (8px grid with 4px micro-steps)
// =============================================================================

/// Horizontal padding for list item inner content
const ITEM_PADDING_X: f32 = 14.0;
/// Vertical padding for list item inner content
const ITEM_PADDING_Y: f32 = 4.0;
/// Right padding on outer container
const ITEM_CONTAINER_PADDING_R: f32 = 4.0;
/// Gap between icon and text content
const ITEM_ICON_TEXT_GAP: f32 = 8.0;
/// Gap between name and description lines
const ITEM_NAME_DESC_GAP: f32 = 2.0;
/// Gap between accessory items (badge, tag, hint)
const ITEM_ACCESSORIES_GAP: f32 = 6.0;
// =============================================================================
// Icon Dimensions
// =============================================================================

/// Icon container size (width and height)
const ICON_CONTAINER_SIZE: f32 = 20.0;
/// SVG icon render size (inside the container)
const ICON_SVG_SIZE: f32 = 16.0;
// =============================================================================
// Typography — font sizes & line heights
// =============================================================================

/// Item name font size (14px — dense desktop default for primary labels)
pub(crate) const NAME_FONT_SIZE: f32 = 14.0;
/// Item name line height
pub(crate) const NAME_LINE_HEIGHT: f32 = 20.0;
/// Item description font size (12px — minimum for desktop legibility)
const DESC_FONT_SIZE: f32 = 12.0;
/// Item description line height (16px fits better within 40px item height)
const DESC_LINE_HEIGHT: f32 = 16.0;
/// Keyboard shortcut badge font size
const BADGE_FONT_SIZE: f32 = 11.0;
/// Search-mode shortcut font size (kept compact to reduce clutter)
const SEARCH_SHORTCUT_FONT_SIZE: f32 = 10.0;
/// Search-mode type accessory icon size (Lucide hint rendered at accent tint)
const TYPE_ACCESSORY_ICON_SIZE: f32 = 12.0;
/// Section header label font size
const SECTION_HEADER_FONT_SIZE: f32 = 12.0;
// =============================================================================
// Badge & Tag Spacing
// =============================================================================

/// Shortcut badge horizontal padding
const BADGE_PADDING_X: f32 = 6.0;
/// Shortcut badge vertical padding
const BADGE_PADDING_Y: f32 = 2.0;
/// Shortcut badge corner radius
const BADGE_RADIUS: f32 = 4.0;
// =============================================================================
// Section Header Spacing
// =============================================================================

/// Section header top padding (visual separation from above item)
pub(crate) const SECTION_PADDING_TOP: f32 = crate::designs::MAIN_MENU_SECTION_PADDING_TOP;
// =============================================================================
// Opacity tokens — named for intent, hex for GPUI rgba() bit-packing
// =============================================================================

/// 85% opacity — used for selected description text
const ALPHA_STRONG: u32 = 0xD9;
/// 65% opacity — focused description: readable but secondary to name
pub(crate) const ALPHA_DESC_SELECTED: u32 = 0xA6;
/// 100% opacity — names are always pure white (Raycast pattern)
pub(crate) const ALPHA_NAME_QUIET: u32 = 0xFF;
/// 50% opacity — used for non-selected item icons
/// Low enough that icons don't compete for attention; selected items restore full color
const ALPHA_ICON_QUIET: u32 = 0x80;
/// 80% opacity — used for shortcut badge text and position indicator
pub(crate) const ALPHA_READABLE: u32 = 0xCC;
/// 75% opacity — used for header icon, tool badge text
/// (Bumped from 70% for better legibility on vibrancy backgrounds)
const ALPHA_MUTED: u32 = 0xBF;
/// 45% opacity — hovered description: visible but clearly lighter than focused
const ALPHA_DESC_QUIET: u32 = 0x73;
/// 70% opacity — used for source hint text
/// (Bumped from 65% for WCAG-friendlier contrast on vibrancy)
const ALPHA_HINT: u32 = 0xB3;
/// 75% opacity — subtle matched-character tint without high-contrast flash
const ALPHA_MATCH_HIGHLIGHT: u32 = 0xBF;
/// 65% opacity — used for section header count
/// (Bumped from 60% for better readability of numeric info)
const ALPHA_SUBTLE: u32 = 0xA6;
/// 30% opacity — used for shortcut badge border
/// (Bumped from 25% so kbd borders pass ≥3:1 non-text contrast on vibrancy)
const ALPHA_BORDER: u32 = 0x4D;
/// 22% opacity — reserved for strong separators (currently unused, kept for design variants)
const _ALPHA_SEPARATOR_STRONG: u32 = 0x38;
/// 8% opacity — used for section separator
/// Barely-there divider; the section label itself provides enough grouping signal
pub(crate) const ALPHA_SEPARATOR: u32 = 0x14;
/// 7% opacity — used for tool badge background
/// (Bumped from 6% for slightly more visible badge pills)
const ALPHA_TINT_MEDIUM: u32 = 0x12;
/// 6% opacity — used for shortcut badge background
/// (Bumped from 5% for subtle but visible kbd backgrounds)
const ALPHA_TINT_LIGHT: u32 = 0x0F;
/// 7% opacity — used for section header background tint
/// (Bumped from 5% for more visible section grouping)
const ALPHA_TINT_FAINT: u32 = 0x12;
// =============================================================================
// Header Area Constants (Ask AI button, shortcut badge, indicators)
// =============================================================================

/// Gap between "Ask AI" text and shortcut badge
pub(crate) const ASK_AI_BUTTON_GAP: f32 = 6.0;
/// Ask AI button horizontal padding
pub(crate) const ASK_AI_BUTTON_PADDING_X: f32 = 6.0;
/// Ask AI button vertical padding
pub(crate) const ASK_AI_BUTTON_PADDING_Y: f32 = 4.0;
/// Ask AI button corner radius
pub(crate) const ASK_AI_BUTTON_RADIUS: f32 = 4.0;
/// Shortcut badge horizontal padding
pub(crate) const TAB_BADGE_PADDING_X: f32 = 6.0;
/// Shortcut badge vertical padding
pub(crate) const TAB_BADGE_PADDING_Y: f32 = 2.0;
/// Shortcut badge corner radius
pub(crate) const TAB_BADGE_RADIUS: f32 = 4.0;
/// 18% opacity — hover accent background on interactive buttons
pub(crate) const ALPHA_HOVER_ACCENT: u32 = 0x2e;
/// 30% opacity — shortcut badge background tint
pub(crate) const ALPHA_TAB_BADGE_BG: u32 = 0x4D;
/// 80% opacity — library size count hint (boosted for vibrancy readability)
pub(crate) const ALPHA_COUNT_HINT: u32 = 0xCC;
// =============================================================================
// Divider & Scroll Constants
// =============================================================================

/// Default horizontal margin for the header/list divider
pub(crate) const DIVIDER_MARGIN_DEFAULT: f32 = 16.0;
/// Default border width for the header/list divider (1px hairline)
pub(crate) const DIVIDER_BORDER_WIDTH_DEFAULT: f32 = 1.0;
/// Maximum height for the log panel area
pub(crate) const LOG_PANEL_MAX_HEIGHT: f32 = 120.0;
/// 38% opacity — divider line between header and list (shared with ui_foundation)
pub(crate) const ALPHA_DIVIDER: u32 = crate::ui_foundation::ALPHA_DIVIDER as u32;
/// Estimated visible list container height for scrollbar calculations
/// Window is 500px, header is ~60px, footer is ~34px → ~406px for list area
pub(crate) const ESTIMATED_LIST_CONTAINER_HEIGHT: f32 = 436.0;
/// Average item height for scroll-wheel delta-to-index conversion
/// Weighted: most items are 40px (LIST_ITEM_HEIGHT), headers are 32px (SECTION_HEADER_HEIGHT)
pub(crate) const AVERAGE_ITEM_HEIGHT_FOR_SCROLL: f32 = 44.0;
// =============================================================================
// Font Family Tokens
// =============================================================================

/// System UI font for all list item text
pub(crate) const FONT_SYSTEM_UI: &str = ".AppleSystemUIFont";
/// Monospace font for keyboard shortcuts and code badges
pub(crate) const FONT_MONO: &str = "JetBrains Mono";
/// Enum for grouped list items - supports both regular items and section headers
///
/// Used with GPUI's `list()` component when rendering grouped results (e.g., frecency with RECENT/MAIN sections).
/// The usize in Item variant is the index into the flat results array.
#[derive(Clone, Debug)]
pub struct SourceChipStatusRow {
    pub source: crate::menu_syntax::RootUnifiedSourceFilter,
    pub source_name: String,
    pub status_kind: SourceChipStatusKind,
    pub label: String,
    pub shown: usize,
    pub loaded: usize,
    pub total: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceChipStatusKind {
    Showing,
    Loading,
    Exhausted,
    Disabled,
    ProviderUnavailable,
}

impl SourceChipStatusKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Showing => "showing",
            Self::Loading => "loading",
            Self::Exhausted => "exhausted",
            Self::Disabled => "disabled",
            Self::ProviderUnavailable => "providerUnavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowEligibility {
    pub focusable: bool,
    pub selectable: bool,
    pub activatable: bool,
}

impl RowEligibility {
    pub const fn inert() -> Self {
        Self {
            focusable: false,
            selectable: false,
            activatable: false,
        }
    }

    pub const fn enabled_action() -> Self {
        Self {
            focusable: true,
            selectable: true,
            activatable: true,
        }
    }

    pub const fn disabled_explanation() -> Self {
        Self {
            focusable: true,
            selectable: true,
            activatable: false,
        }
    }

    pub fn new(focusable: bool, selectable: bool, activatable: bool) -> Self {
        assert!(
            !activatable || selectable,
            "activatable rows must be selectable"
        );
        assert!(
            !selectable || focusable,
            "selectable rows must be focusable"
        );
        Self {
            focusable,
            selectable,
            activatable,
        }
    }
}

pub fn first_selectable_eligibility_index(rows: &[RowEligibility]) -> Option<usize> {
    rows.iter().position(|row| row.selectable)
}

pub fn last_selectable_eligibility_index(rows: &[RowEligibility]) -> Option<usize> {
    rows.iter().rposition(|row| row.selectable)
}

pub fn selectable_eligibility_index_at_or_before(
    rows: &[RowEligibility],
    start: usize,
) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }
    let clamped = start.min(rows.len() - 1);
    (0..=clamped).rev().find(|&ix| rows[ix].selectable)
}

pub fn selectable_eligibility_index_at_or_after(
    rows: &[RowEligibility],
    start: usize,
) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }
    let clamped = start.min(rows.len() - 1);
    (clamped..rows.len()).find(|&ix| rows[ix].selectable)
}

pub fn coerce_eligibility_selection(rows: &[RowEligibility], index: usize) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }
    let index = index.min(rows.len() - 1);
    if rows[index].selectable {
        return Some(index);
    }
    selectable_eligibility_index_at_or_after(rows, index.saturating_add(1))
        .or_else(|| selectable_eligibility_index_at_or_before(rows, index.saturating_sub(1)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionPresentationFamily {
    /// Launcher-family lists use presentation-only uppercase and strong label emphasis.
    Launcher,
    /// Other list families preserve authored casing and their quieter existing emphasis.
    PreserveAuthored,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionTextTier {
    Strong,
    Muted,
}

impl SectionTextTier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strong => "strong",
            Self::Muted => "muted",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionHeaderPresentation {
    /// The exact authored source. Presentation transforms must never mutate this value.
    pub semantic_label: SharedString,
    pub display_label: SharedString,
    pub count: Option<SharedString>,
    pub icon: Option<SharedString>,
    pub label_tier: SectionTextTier,
    pub count_tier: SectionTextTier,
    pub icon_tier: SectionTextTier,
}

/// Resolve section copy and emphasis without changing its semantic source.
pub fn resolve_section_header_presentation(
    label: &str,
    icon: Option<&str>,
    count: Option<&str>,
    family: SectionPresentationFamily,
) -> SectionHeaderPresentation {
    let semantic_label: SharedString = label.to_string().into();
    let display_label = match family {
        SectionPresentationFamily::Launcher => label.to_uppercase().into(),
        SectionPresentationFamily::PreserveAuthored => semantic_label.clone(),
    };
    SectionHeaderPresentation {
        semantic_label,
        display_label,
        count: count.map(|value| value.to_string().into()),
        icon: icon.map(|value| value.to_string().into()),
        label_tier: match family {
            SectionPresentationFamily::Launcher => SectionTextTier::Strong,
            SectionPresentationFamily::PreserveAuthored => SectionTextTier::Muted,
        },
        count_tier: SectionTextTier::Muted,
        icon_tier: SectionTextTier::Muted,
    }
}

#[derive(Clone, Debug)]
pub enum GroupedListItem {
    /// A semantic section header with authored source text and optional icon.
    SectionHeader(String, Option<String>),
    /// A visual first-section slot with no semantic heading or accessible label.
    ReservedSectionSlot,
    /// Transient source-chip status metadata produced during grouping.
    Status(SourceChipStatusRow),
    /// A regular list item - usize is the index in the flat results array
    Item(usize),
}

/// Reserve row zero for launcher section chrome without inventing an empty heading.
pub fn ensure_launcher_section_slot(items: &mut Vec<GroupedListItem>) {
    let has_result = items
        .iter()
        .any(|item| matches!(item, GroupedListItem::Item(_)));
    if has_result
        && !matches!(
            items.first(),
            Some(GroupedListItem::SectionHeader(..) | GroupedListItem::ReservedSectionSlot)
        )
    {
        items.insert(0, GroupedListItem::ReservedSectionSlot);
    }
}

pub fn grouped_list_item_eligibility(item: &GroupedListItem) -> RowEligibility {
    match item {
        GroupedListItem::Item(_) => RowEligibility::enabled_action(),
        GroupedListItem::SectionHeader(..)
        | GroupedListItem::ReservedSectionSlot
        | GroupedListItem::Status(_) => RowEligibility::inert(),
    }
}

/// Coerce a selection index to land on a selectable (non-header) row.
///
/// When the given index lands on a header or is out of bounds:
/// 1. First tries searching DOWN to find the next Item
/// 2. If not found, searches UP to find the previous Item
/// 3. If still not found (list has no items), returns None
///
/// This is the canonical way to ensure selection never lands on a header.
///
/// # Performance
/// O(n) worst case, but typically O(1) since headers are sparse.
///
/// # Returns
/// - `Some(index)` - Valid selectable index
/// - `None` - No selectable items exist (list is empty or contains only headers)
pub fn coerce_selection(rows: &[GroupedListItem], ix: usize) -> Option<usize> {
    let eligibility: Vec<_> = rows.iter().map(grouped_list_item_eligibility).collect();
    coerce_eligibility_selection(&eligibility, ix)
}
/// Pre-computed grouped list state for efficient navigation
///
/// This struct caches header positions and total counts to avoid expensive
/// recalculation on every keypress. Build it once when the list data changes,
/// then reuse for navigation.
///
/// ## Performance
/// - `is_header()`: O(1) lookup via HashSet
/// - `next_selectable()` / `prev_selectable()`: O(k) where k is consecutive headers
/// - Memory: O(h) where h is number of headers (typically < 10)
///
#[derive(Clone, Debug)]
pub struct GroupedListState {
    /// Set of indices that are headers (for O(1) lookup)
    header_indices: std::collections::HashSet<usize>,
    /// Total number of visual items (headers + entries)
    pub total_items: usize,
    /// Index of first selectable item (skips leading header)
    pub first_selectable: usize,
}
impl GroupedListState {
    /// Create from a list of (group_name, item_count) pairs
    ///
    /// Each group gets a header at the start, followed by its items.
    /// Empty groups are skipped (no header for empty groups).
    pub fn from_groups(groups: &[(&str, usize)]) -> Self {
        let mut header_indices = std::collections::HashSet::new();
        let mut idx = 0;

        for (_, count) in groups {
            if *count > 0 {
                header_indices.insert(idx); // Header position
                idx += 1 + count; // Header + items
            }
        }

        let first_selectable = if header_indices.contains(&0) { 1 } else { 0 };

        Self {
            header_indices,
            total_items: idx,
            first_selectable,
        }
    }

    /// Create from pre-built GroupedListItem vec (when you already have the items)
    pub fn from_items(items: &[GroupedListItem]) -> Self {
        let mut header_indices = std::collections::HashSet::new();

        for (idx, item) in items.iter().enumerate() {
            if matches!(
                item,
                GroupedListItem::SectionHeader(..) | GroupedListItem::ReservedSectionSlot
            ) {
                header_indices.insert(idx);
            }
        }

        let first_selectable = if header_indices.contains(&0) { 1 } else { 0 };

        Self {
            header_indices,
            total_items: items.len(),
            first_selectable,
        }
    }

    /// Create an empty state (no headers, for flat lists)
    pub fn flat(item_count: usize) -> Self {
        Self {
            header_indices: std::collections::HashSet::new(),
            total_items: item_count,
            first_selectable: 0,
        }
    }

    /// Check if an index is a header (O(1))
    #[inline]
    pub fn is_header(&self, index: usize) -> bool {
        self.header_indices.contains(&index)
    }

    /// Get next selectable index (skips headers), or None if at end
    pub fn next_selectable(&self, current: usize) -> Option<usize> {
        let mut next = current + 1;
        while next < self.total_items && self.is_header(next) {
            next += 1;
        }
        if next < self.total_items {
            Some(next)
        } else {
            None
        }
    }

    /// Get previous selectable index (skips headers), or None if at start
    pub fn prev_selectable(&self, current: usize) -> Option<usize> {
        if current == 0 {
            return None;
        }
        let mut prev = current - 1;
        while prev > 0 && self.is_header(prev) {
            prev -= 1;
        }
        if !self.is_header(prev) {
            Some(prev)
        } else {
            None
        }
    }

    /// Get number of headers
    pub fn header_count(&self) -> usize {
        self.header_indices.len()
    }
}
/// Pre-computed colors for ListItem rendering
///
/// This struct holds the primitive color values needed for list item rendering,
/// allowing efficient use in closures without cloning the full theme.
#[derive(Clone, Copy)]
pub struct ListItemColors {
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub text_dimmed: u32,
    pub accent_selected: u32,
    pub accent_selected_subtle: u32,
    pub background: u32,
    pub background_selected: u32,
    /// Opacity for selected item background (from theme.opacity.selected)
    pub selected_opacity: f32,
    /// Opacity for hovered item background (from theme.opacity.hover)
    pub hover_opacity: f32,
    /// Warning background color (for confirmation overlays, alerts)
    pub warning_bg: u32,
    /// Text color for content displayed on accent/warning backgrounds
    pub text_on_accent: u32,
    // ── Text grading alphas (u32, 0x00–0xFF) ────────────────────────────
    // All applied to text_primary. Sourced from theme.opacity text tiers.
    /// Name text alpha (idle items).
    pub alpha_name: u32,
    /// Badge / shortcut / section header alpha.
    pub alpha_strong: u32,
    /// Focused description / source hint alpha.
    pub alpha_muted: u32,
    /// Hovered description / type label alpha.
    pub alpha_hint: u32,
    /// Placeholder / idle caption alpha.
    pub alpha_placeholder: u32,
    /// Idle icon alpha.
    pub alpha_icon: u32,
}

#[inline]
pub fn row_selected_background_rgba(colors: &ListItemColors) -> u32 {
    let selected_alpha = (colors.selected_opacity * 255.0) as u8;
    (colors.text_primary << 8) | selected_alpha as u32
}

#[inline]
pub fn row_hover_background_rgba(colors: &ListItemColors) -> u32 {
    let hover_alpha = (colors.hover_opacity * 255.0) as u8;
    (colors.text_primary << 8) | hover_alpha as u32
}

#[inline]
pub fn row_name_text_rgba(colors: &ListItemColors, selected: bool) -> u32 {
    if selected {
        (colors.text_primary << 8) | 0xFF
    } else {
        (colors.text_primary << 8) | colors.alpha_name
    }
}

#[inline]
pub fn row_description_text_rgba(colors: &ListItemColors, selected: bool) -> u32 {
    if selected {
        (colors.text_primary << 8) | colors.alpha_muted
    } else {
        (colors.text_primary << 8) | colors.alpha_hint
    }
}

#[inline]
pub fn row_icon_text_rgba(colors: &ListItemColors, selected: bool) -> u32 {
    if selected {
        (colors.text_primary << 8) | 0xFF
    } else {
        (colors.text_primary << 8) | colors.alpha_icon
    }
}

#[inline]
pub fn row_type_accessory_rgba(colors: &ListItemColors, selected: bool) -> u32 {
    let alpha = if selected {
        colors.alpha_strong
    } else {
        colors.alpha_icon
    };
    (colors.accent_selected << 8) | alpha
}

#[cfg(test)]
mod icon_kind_tests {
    use super::IconKind;

    #[test]
    fn test_icon_kind_from_icon_hint_returns_svg_when_known_icon_name() {
        match IconKind::from_icon_hint("terminal") {
            Some(IconKind::Svg(name)) => assert_eq!(name, "terminal"),
            _ => panic!("expected SVG icon from known icon hint"),
        }
    }

    #[test]
    fn test_icon_kind_from_icon_hint_returns_emoji_when_symbol_glyph() {
        match IconKind::from_icon_hint("📄") {
            Some(IconKind::Emoji(emoji)) => assert_eq!(emoji, "📄"),
            _ => panic!("expected emoji icon for symbol glyph"),
        }
    }

    #[test]
    fn test_icon_kind_from_icon_hint_returns_none_for_unknown_ascii_word() {
        assert!(IconKind::from_icon_hint("unknown-icon-name").is_none());
    }
}

#[cfg(test)]
mod list_item_colors_tests {
    use super::{ListItemColors, ListItemMetricsOverride, ALPHA_DIVIDER};

    #[test]
    fn test_from_theme_sets_text_on_accent_from_theme_text_on_accent() {
        let mut theme = crate::theme::Theme::default();
        theme.colors.text.primary = 0x010203;
        theme.colors.text.on_accent = 0xa1b2c3;

        let colors = ListItemColors::from_theme(&theme);

        assert_eq!(colors.text_on_accent, theme.colors.text.on_accent);
        assert_ne!(colors.text_on_accent, theme.colors.text.primary);
    }

    #[test]
    fn test_from_design_with_dark_mode_uses_theme_row_opacity_ladders() {
        let design = crate::designs::DesignColors::default();
        let dark = ListItemColors::from_design_with_dark_mode(&design, true);
        let light = ListItemColors::from_design_with_dark_mode(&design, false);
        let dark_opacity = crate::theme::types::BackgroundOpacity::dark_default();
        let light_opacity = crate::theme::types::BackgroundOpacity::light_default();

        assert_eq!(dark.selected_opacity, dark_opacity.selected);
        assert_eq!(dark.hover_opacity, dark_opacity.hover);
        assert_eq!(light.selected_opacity, light_opacity.selected);
        assert_eq!(light.hover_opacity, light_opacity.hover);
    }

    #[test]
    fn test_alpha_divider_matches_ui_foundation_constant() {
        assert_eq!(ALPHA_DIVIDER, crate::ui_foundation::ALPHA_DIVIDER as u32);
    }

    #[test]
    fn list_item_render_inputs_resolve_through_canonical_state_palette() {
        let theme = crate::theme::Theme::dark_default();
        let mut colors = ListItemColors::from_theme(&theme);
        colors.text_primary = 0x102030;
        colors.accent_selected = 0x405060;
        colors.text_on_accent = 0xF0E0D0;
        colors.alpha_name = 0xA1;
        colors.hover_opacity = 0.22;
        let mut metrics = ListItemMetricsOverride::from_main_menu_theme(
            crate::designs::MainMenuThemeVariant::InfoBarBase,
        );
        metrics.row_hover_fill_alpha = 0x12;
        metrics.row_selected_fill_alpha = 0x20;

        let palette = crate::theme::resolve_main_menu_row_state_palette_from_parts(
            crate::theme::MainMenuRowColorInputs {
                row_kind: crate::designs::MainMenuRowKind::IconTile,
                row_hover_fill_alpha: metrics.row_hover_fill_alpha as u8,
                row_selected_fill_alpha: metrics.row_selected_fill_alpha as u8,
                theme_hover_opacity: colors.hover_opacity,
                text_primary_hex: colors.text_primary,
                accent_selected_hex: colors.accent_selected,
                text_on_accent_hex: colors.text_on_accent,
                primary_name_alpha: colors.alpha_name as u8,
            },
        );

        assert_eq!(palette.rest.primary_foreground_rgba, 0x102030A1);
        assert_eq!(palette.hover.background_rgba, Some(0x10203038));
        assert_eq!(palette.active.background_rgba, Some(0x10203020));
        assert_eq!(palette.active.primary_foreground_rgba, 0x102030FF);
    }
}

#[cfg(test)]
mod selection_marker_tests {
    use super::{list_item_selection_marker_geometry, ListItemMetricsOverride};

    #[test]
    fn approved_marker_is_an_absolute_overlay_inside_the_row_surface() {
        let metrics = ListItemMetricsOverride::from_main_menu_theme(
            crate::designs::MainMenuThemeVariant::InfoBarBase,
        );
        let marker = list_item_selection_marker_geometry(metrics, true)
            .expect("selected launcher rows paint a marker");

        assert_eq!(marker.left, metrics.row_outer_padding_x + 6.0);
        assert_eq!(
            marker.top,
            (metrics.row_selected_marker_center_height - 16.0) / 2.0
        );
        assert_eq!(marker.width, 2.0);
        assert_eq!(marker.height, 16.0);
        assert_eq!(marker.radius, 1.0);
        assert_eq!(marker.alpha, 0xFF);
        assert!(marker.left >= metrics.row_outer_padding_x);
        assert!(marker.top >= metrics.row_outer_padding_y);
        assert!(marker.top + marker.height <= metrics.item_height - metrics.row_outer_padding_y);
    }

    #[test]
    fn marker_visibility_depends_only_on_selected_location() {
        let metrics = ListItemMetricsOverride::default_main_menu();
        assert!(list_item_selection_marker_geometry(metrics, true).is_some());
        assert!(list_item_selection_marker_geometry(metrics, false).is_none());
    }

    #[test]
    fn marker_tokens_do_not_change_row_content_geometry() {
        let mut changed_marker = ListItemMetricsOverride::from_main_menu_theme(
            crate::designs::MainMenuThemeVariant::InfoBarBase,
        );
        let baseline = changed_marker;
        changed_marker.row_selected_marker_width = 3.0;
        changed_marker.row_selected_marker_height = 18.0;
        changed_marker.row_selected_marker_inset_x = 7.0;

        assert_eq!(changed_marker.item_height, baseline.item_height);
        assert_eq!(
            changed_marker.row_outer_padding_x,
            baseline.row_outer_padding_x
        );
        assert_eq!(
            changed_marker.row_outer_padding_y,
            baseline.row_outer_padding_y
        );
        assert_eq!(
            changed_marker.row_inner_padding_x,
            baseline.row_inner_padding_x
        );
        assert_eq!(
            changed_marker.row_inner_padding_y,
            baseline.row_inner_padding_y
        );
        assert_eq!(
            changed_marker.icon_container_size,
            baseline.icon_container_size
        );
        assert_eq!(changed_marker.icon_text_gap, baseline.icon_text_gap);
        assert_eq!(changed_marker.accessory_gap, baseline.accessory_gap);
    }
}

#[cfg(test)]
mod row_chrome_rgba_tests {
    use super::{row_type_accessory_rgba, ListItemColors};

    #[test]
    fn test_row_type_accessory_rgba_uses_theme_icon_and_strong_alphas() {
        let mut theme = crate::theme::Theme::dark_default();
        theme.opacity = Some(crate::theme::types::BackgroundOpacity {
            text_icon: 0.42,
            text_strong: 0.88,
            ..theme.get_opacity()
        });
        let colors = ListItemColors::from_theme(&theme);

        let idle = row_type_accessory_rgba(&colors, false);
        let selected = row_type_accessory_rgba(&colors, true);

        assert_eq!(idle & 0xFF, colors.alpha_icon as u32);
        assert_eq!(selected & 0xFF, colors.alpha_strong as u32);
        assert_eq!(idle >> 8, colors.accent_selected);
    }
}

#[cfg(test)]
mod row_shortcut_policy_tests {
    use super::{
        should_show_row_shortcut, should_show_search_shortcut, RowShortcutVisibilityPolicy,
    };

    #[test]
    fn selected_only_shows_shortcut_on_focused_row() {
        let p = RowShortcutVisibilityPolicy::SelectedOnly;
        assert!(should_show_row_shortcut(p, true, false));
        assert!(should_show_row_shortcut(p, true, true));
    }

    #[test]
    fn selected_only_hides_shortcut_on_unfocused_row() {
        let p = RowShortcutVisibilityPolicy::SelectedOnly;
        assert!(!should_show_row_shortcut(p, false, false));
        assert!(!should_show_row_shortcut(p, false, true));
    }

    #[test]
    fn all_rows_always_shows_shortcut() {
        let p = RowShortcutVisibilityPolicy::AllRows;
        assert!(should_show_row_shortcut(p, true, false));
        assert!(should_show_row_shortcut(p, true, true));
        assert!(should_show_row_shortcut(p, false, false));
        assert!(should_show_row_shortcut(p, false, true));
    }

    #[test]
    fn search_shortcut_delegates_to_selected_only() {
        // Dense launcher rows use SelectedOnly — only selected rows show shortcuts.
        assert!(should_show_search_shortcut(true, true, false));
        assert!(!should_show_search_shortcut(true, false, false));
        assert!(!should_show_search_shortcut(false, false, false));
    }
}

impl ListItemColors {
    /// Create from theme reference
    pub fn from_theme(theme: &crate::theme::Theme) -> Self {
        let opacity = theme.get_opacity();
        Self {
            text_primary: theme.colors.text.primary,
            text_secondary: theme.colors.text.secondary,
            text_muted: theme.colors.text.muted,
            text_dimmed: theme.colors.text.dimmed,
            accent_selected: theme.colors.accent.selected,
            accent_selected_subtle: theme.colors.accent.selected_subtle,
            background: theme.colors.background.main,
            background_selected: theme.colors.accent.selected_subtle,
            selected_opacity: opacity.selected,
            hover_opacity: opacity.hover,
            warning_bg: theme.colors.ui.warning,
            text_on_accent: theme.colors.text.on_accent,
            // Text grading alphas from theme
            alpha_name: crate::theme::types::opacity_to_alpha(opacity.text_name),
            alpha_strong: crate::theme::types::opacity_to_alpha(opacity.text_strong),
            alpha_muted: crate::theme::types::opacity_to_alpha(opacity.text_muted_alpha),
            alpha_hint: crate::theme::types::opacity_to_alpha(opacity.text_hint),
            alpha_placeholder: crate::theme::types::opacity_to_alpha(opacity.text_placeholder),
            alpha_icon: crate::theme::types::opacity_to_alpha(opacity.text_icon),
        }
    }

    /// Create from design colors for GLOBAL theming support
    /// Uses same opacity values as from_theme() for consistent vibrancy-compatible styling
    ///
    /// NOTE: This defaults to dark mode opacity values. For light mode support,
    /// use `from_design_with_dark_mode()` instead.
    pub fn from_design(colors: &crate::designs::DesignColors) -> Self {
        // Default to dark mode
        Self::from_design_with_dark_mode(colors, true)
    }

    /// Create from design colors with explicit dark/light mode
    ///
    /// Row-state opacity comes from the same appearance-aware defaults used by
    /// normal themes, keeping design previews aligned with the app shell.
    ///
    /// # Arguments
    /// * `colors` - Design colors to use
    /// * `is_dark` - True for dark mode (lower opacity), false for light mode (higher opacity)
    pub fn from_design_with_dark_mode(
        colors: &crate::designs::DesignColors,
        is_dark: bool,
    ) -> Self {
        let opacity = if is_dark {
            crate::theme::types::BackgroundOpacity::dark_default()
        } else {
            crate::theme::types::BackgroundOpacity::light_default()
        };

        Self {
            text_primary: colors.text_primary,
            text_secondary: colors.text_secondary,
            text_muted: colors.text_muted,
            text_dimmed: colors.text_dimmed,
            accent_selected: colors.accent,
            accent_selected_subtle: colors.background_selected,
            background: colors.background,
            background_selected: colors.background_selected,
            selected_opacity: opacity.selected,
            hover_opacity: opacity.hover,
            warning_bg: colors.warning,
            text_on_accent: colors.text_on_accent,
            // Use defaults for design colors (no theme opacity override available)
            alpha_name: crate::theme::types::opacity_to_alpha(1.0),
            alpha_strong: crate::theme::types::opacity_to_alpha(0.80),
            alpha_muted: crate::theme::types::opacity_to_alpha(0.65),
            alpha_hint: crate::theme::types::opacity_to_alpha(0.45),
            alpha_placeholder: crate::theme::types::opacity_to_alpha(0.40),
            alpha_icon: crate::theme::types::opacity_to_alpha(0.50),
        }
    }
}
/// Format a keyboard shortcut string using macOS-native modifier symbols.
///
/// Converts common shortcut formats to native macOS symbols:
/// - "cmd+shift+k" → "⌘⇧K"
/// - "ctrl+c" → "⌃C"
/// - "alt+enter" → "⌥↩"
///
/// Delegates to the shared hint_strip normalizer to prevent mapping drift.
/// Preserves legacy `↩` output for plain-string callers by replacing `↵` → `↩`.
pub fn format_shortcut_display(shortcut: &str) -> String {
    let display =
        crate::components::hint_strip::compact_shortcut_display_string(shortcut).replace('↵', "↩");
    crate::components::hint_strip::emit_shortcut_normalization_audit(
        "list_item_format",
        shortcut,
        &display,
    );
    display
}
/// Resolve shortcut tokens for render, preferring pre-cached tokens.
/// Falls back to on-demand parsing when tokens are missing. This helper runs
/// in the render path, so it must stay side-effect free.
fn list_item_shortcut_tokens_for_render<'a>(
    shortcut: Option<&'a str>,
    shortcut_tokens: Option<&'a [String]>,
) -> Option<std::borrow::Cow<'a, [String]>> {
    if let Some(tokens) = shortcut_tokens {
        return Some(std::borrow::Cow::Borrowed(tokens));
    }
    let shortcut = shortcut?;
    Some(std::borrow::Cow::Owned(
        crate::components::hint_strip::shortcut_tokens_from_hint(shortcut),
    ))
}

/// Explicit policy for when row shortcut chrome is visible.
///
/// Dense launcher-style lists use `SelectedOnly` so shortcuts appear only on
/// the focused row (quiet chrome). Discovery surfaces like the actions dialog
/// use `AllRows` so every row exposes its shortcut at hint opacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowShortcutVisibilityPolicy {
    /// Shortcuts visible only on the selected (focused) row.
    SelectedOnly,
    /// Shortcuts visible on every row at hint opacity.
    AllRows,
}

/// Resolve whether a row should render its shortcut chrome.
#[inline]
pub(crate) fn should_show_row_shortcut(
    policy: RowShortcutVisibilityPolicy,
    selected: bool,
    _hovered: bool,
) -> bool {
    match policy {
        RowShortcutVisibilityPolicy::SelectedOnly => selected,
        RowShortcutVisibilityPolicy::AllRows => true,
    }
}

/// Dense launcher rows keep shortcut chrome on the focused row only.
/// Focus controls description reveal, not metadata discoverability.
pub(crate) fn should_show_search_shortcut(
    _is_filtering: bool,
    selected: bool,
    _hovered: bool,
) -> bool {
    should_show_row_shortcut(RowShortcutVisibilityPolicy::SelectedOnly, selected, false)
}
/// Search rows keep descriptions only when they add context for the current focus or match.
pub(crate) fn should_show_search_description(
    selected: bool,
    hovered: bool,
    has_description_match: bool,
) -> bool {
    selected || hovered || has_description_match
}
/// Callback type for hover events on list items.
/// The callback receives the item index and a boolean indicating hover state (true = entered, false = left).
pub type OnHoverCallback = Box<dyn Fn(usize, bool) + 'static>;
/// A reusable list item component for displaying selectable items
///
/// Supports:
/// - Name (required)
/// - Description (optional, shown below name)
/// - Icon (optional, emoji or PNG image displayed left of name)
/// - Shortcut badge (optional, right-aligned)
/// - Selection state with themed colors (full focus styling)
/// - Hover state with subtle visual feedback (separate from selection)
/// - Hover callback for mouse interaction (optional)
/// - Semantic ID for AI-driven targeting (optional)
///
#[derive(IntoElement)]
pub struct ListItem {
    name: SharedString,
    description: Option<String>,
    shortcut: Option<String>,
    shortcut_tokens: Option<Vec<String>>,
    shortcut_visibility_policy: RowShortcutVisibilityPolicy,
    icon: Option<IconKind>,
    destructive_text_color: Option<u32>,
    selected: bool,
    disabled: bool,
    /// Whether this item is being hovered (subtle visual feedback, separate from selected)
    hovered: bool,
    colors: ListItemColors,
    /// Index of this item in the list (needed for hover callback)
    index: Option<usize>,
    /// Optional callback triggered when mouse enters/leaves this item
    on_hover: Option<OnHoverCallback>,
    /// Semantic ID for AI-driven UX targeting. Format: {type}:{index}:{value}
    semantic_id: Option<String>,
    /// Live cohesive theme exploration variation (main menu).
    main_menu_theme: crate::designs::MainMenuThemeVariant,
    /// Optional metrics override for shared-row consumers that are not the main menu.
    metrics_override: Option<ListItemMetricsOverride>,
    /// Character indices in the name that match the search query (for fuzzy highlight)
    /// When present, matched characters are rendered with accent color for visual emphasis
    highlight_indices: Option<Vec<usize>>,
    /// Character indices in the description that match the search query (for fuzzy highlight)
    /// When present, matched characters are rendered with accent color for visual emphasis
    description_highlight_indices: Option<Vec<usize>>,
    /// Type accessory shown as a subtle accent-tinted icon during search mode
    /// to help distinguish mixed result types without visible type text.
    type_accessory: Option<TypeAccessory>,
    /// Source/kit name (e.g., "main", "cleanshot") shown as subtle text during search
    source_hint: Option<String>,
    /// Tool/language badge for scriptlets (e.g., "ts", "bash", "paste")
    /// Shown as a subtle monospace badge in the accessories area
    tool_badge: Option<String>,
    /// Generic leading accessory element rendered between the icon and the text content.
    /// Use for domain-specific visuals (e.g., color swatch strips) without coupling
    /// the shared row primitive to any particular consumer.
    leading_accessory: Option<AnyElement>,
    /// Generic trailing accessory element appended after the standard accessories area.
    /// Use for domain-specific badges or indicators (e.g., "Saved" status badges).
    trailing_accessory: Option<AnyElement>,
}
/// Type accessory displayed as a subtle accent-tinted icon on list items during search
#[derive(Clone, Debug)]
pub struct TypeAccessory {
    /// Tooltip/accessibility label (e.g., "Script", "Snippet", "App")
    pub label: &'static str,
    /// Lucide or Script Kit icon hint name (e.g., "file-code", "command")
    pub icon_name: &'static str,
}
/// Rounded selected/hover row radius for Tahoe/Liquid Glass visual hierarchy.
pub const LIST_ITEM_ROW_RADIUS_PX: f32 = crate::ui::chrome::LIQUID_GLASS_COMPACT_RADIUS_PX;

/// Collapse newlines (and surrounding whitespace) into a single space
/// so that list item text always renders on one line.
fn sanitize_newlines(s: String) -> String {
    if s.contains('\n') || s.contains('\r') {
        s.replace("\r\n", " ").replace(['\r', '\n'], " ")
    } else {
        s
    }
}

impl ListItem {
    /// Create a new list item with the given name and pre-computed colors
    pub fn new(name: impl Into<SharedString>, colors: ListItemColors) -> Self {
        let name_str: SharedString = name.into();
        // Collapse newlines to spaces so text stays on a single line in the list
        let name_sanitized: SharedString = if name_str.contains('\n') || name_str.contains('\r') {
            name_str
                .replace("\r\n", " ")
                .replace(['\r', '\n'], " ")
                .into()
        } else {
            name_str
        };
        Self {
            name: name_sanitized,
            description: None,
            shortcut: None,
            shortcut_tokens: None,
            shortcut_visibility_policy: RowShortcutVisibilityPolicy::SelectedOnly,
            icon: None,
            destructive_text_color: None,
            selected: false,
            disabled: false,
            hovered: false,
            colors,
            index: None,
            on_hover: None,
            semantic_id: None,
            main_menu_theme: crate::designs::MainMenuThemeVariant::default(),
            metrics_override: None,
            highlight_indices: None,
            description_highlight_indices: None,
            type_accessory: None,
            source_hint: None,
            tool_badge: None,
            leading_accessory: None,
            trailing_accessory: None,
        }
    }

    /// Set the live cohesive theme exploration variation for this row.
    pub fn main_menu_theme(mut self, theme: crate::designs::MainMenuThemeVariant) -> Self {
        self.main_menu_theme = theme;
        self
    }

    /// Override row metrics while preserving the shared ListItem render anatomy.
    pub fn metrics_override(mut self, metrics: ListItemMetricsOverride) -> Self {
        self.metrics_override = Some(metrics);
        self
    }

    /// Set the index of this item in the list (required for hover callback to work)
    pub fn index(mut self, index: usize) -> Self {
        self.index = Some(index);
        self
    }

    /// Set a callback to be triggered when mouse enters or leaves this item.
    /// The callback receives (index, is_hovered) where is_hovered is true when entering.
    pub fn on_hover(mut self, callback: OnHoverCallback) -> Self {
        self.on_hover = Some(callback);
        self
    }

    /// Set the semantic ID for AI-driven UX targeting.
    /// Format: {type}:{index}:{value} (e.g., "choice:0:apple")
    pub fn semantic_id(mut self, id: impl Into<String>) -> Self {
        self.semantic_id = Some(id.into());
        self
    }

    /// Set an optional semantic ID (convenience for Option<String>)
    pub fn semantic_id_opt(mut self, id: Option<String>) -> Self {
        self.semantic_id = id;
        self
    }

    /// Set the description text (shown below the name)
    pub fn description(mut self, d: impl Into<String>) -> Self {
        let s: String = d.into();
        // Collapse newlines to spaces so description stays on a single line
        self.description = Some(sanitize_newlines(s));
        self
    }

    /// Set an optional description (convenience for Option<String>)
    pub fn description_opt(mut self, d: Option<String>) -> Self {
        self.description = d.map(sanitize_newlines);
        self
    }

    /// Set the shortcut badge text (shown right-aligned)
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        let shortcut = s.into();
        let shortcut_tokens = crate::components::hint_strip::shortcut_tokens_from_hint(&shortcut);
        self.shortcut_tokens = Some(shortcut_tokens);
        self.shortcut = Some(shortcut);
        self
    }

    /// Set an optional shortcut (convenience for Option<String>)
    pub fn shortcut_opt(mut self, s: Option<String>) -> Self {
        if let Some(ref shortcut) = s {
            let shortcut_tokens =
                crate::components::hint_strip::shortcut_tokens_from_hint(shortcut);
            self.shortcut_tokens = Some(shortcut_tokens);
        } else {
            self.shortcut_tokens = None;
        }
        self.shortcut = s;
        self
    }

    /// Set when shortcut chrome should be visible for this row.
    pub(crate) fn shortcut_visibility_policy(
        mut self,
        policy: RowShortcutVisibilityPolicy,
    ) -> Self {
        self.shortcut_visibility_policy = policy;
        self
    }

    /// Render primary row text/icon with a destructive color.
    pub(crate) fn destructive_text_color(mut self, color: u32) -> Self {
        self.destructive_text_color = Some(color);
        self
    }

    /// Set the icon (emoji) to display on the left side
    pub fn icon(mut self, i: impl Into<String>) -> Self {
        self.icon = Some(IconKind::Emoji(i.into()));
        self
    }

    /// Set an optional emoji icon (convenience for Option<String>)
    pub fn icon_opt(mut self, i: Option<String>) -> Self {
        self.icon = i.map(IconKind::Emoji);
        self
    }

    /// Set a pre-decoded RenderImage icon
    pub fn icon_image(mut self, image: Arc<RenderImage>) -> Self {
        self.icon = Some(IconKind::Image(image));
        self
    }

    /// Set an optional pre-decoded image icon
    pub fn icon_image_opt(mut self, image: Option<Arc<RenderImage>>) -> Self {
        self.icon = image.map(IconKind::Image);
        self
    }

    /// Set icon from IconKind enum (for mixed icon types)
    pub fn icon_kind(mut self, kind: IconKind) -> Self {
        self.icon = Some(kind);
        self
    }

    /// Set an optional icon from IconKind
    pub fn icon_kind_opt(mut self, kind: Option<IconKind>) -> Self {
        self.icon = kind;
        self
    }

    /// Set whether this item is selected
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set whether this item is hovered (visual feedback)
    ///
    /// Hovered items show a visible background tint (25% opacity).
    /// This is separate from `selected` which shows full focus styling
    /// (35% opacity background).
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    /// Set character indices for fuzzy match highlighting
    /// When set, matched characters in the name are rendered with accent color
    pub fn highlight_indices(mut self, indices: Vec<usize>) -> Self {
        if !indices.is_empty() {
            self.highlight_indices = Some(indices);
        }
        self
    }

    /// Set optional highlight indices (convenience for Option<Vec<usize>>)
    pub fn highlight_indices_opt(mut self, indices: Option<Vec<usize>>) -> Self {
        self.highlight_indices = indices.filter(|v| !v.is_empty());
        self
    }

    /// Set character indices for fuzzy match highlighting in description
    /// When set, matched characters in the description are rendered with accent color
    pub fn description_highlight_indices(mut self, indices: Vec<usize>) -> Self {
        if !indices.is_empty() {
            self.description_highlight_indices = Some(indices);
        }
        self
    }

    /// Set optional description highlight indices (convenience for Option<Vec<usize>>)
    pub fn description_highlight_indices_opt(mut self, indices: Option<Vec<usize>>) -> Self {
        self.description_highlight_indices = indices.filter(|v| !v.is_empty());
        self
    }

    /// Set a type accessory icon for search-mode rows (label remains available
    /// for accessibility even while list-row hover tooltips are disabled).
    pub fn type_accessory(mut self, accessory: TypeAccessory) -> Self {
        self.type_accessory = Some(accessory);
        self
    }

    /// Set an optional type accessory
    pub fn type_accessory_opt(mut self, accessory: Option<TypeAccessory>) -> Self {
        self.type_accessory = accessory;
        self
    }

    /// Set the source/kit name hint (shown during search to indicate origin)
    pub fn source_hint(mut self, hint: impl Into<String>) -> Self {
        self.source_hint = Some(hint.into());
        self
    }

    /// Set an optional source hint
    pub fn source_hint_opt(mut self, hint: Option<String>) -> Self {
        self.source_hint = hint;
        self
    }

    /// Set the tool/language badge (e.g., "ts", "bash", "paste")
    /// Displayed as a subtle monospace badge for scriptlets
    pub fn tool_badge(mut self, badge: impl Into<String>) -> Self {
        self.tool_badge = Some(badge.into());
        self
    }

    /// Set the tool/language badge from an option
    pub fn tool_badge_opt(mut self, badge: Option<String>) -> Self {
        self.tool_badge = badge;
        self
    }

    /// Set a leading accessory element rendered between the icon and the text content.
    ///
    /// This is a generic slot for domain-specific visuals (e.g., color swatch strips
    /// in a theme chooser). The element is rendered as-is with flex_shrink_0.
    pub fn leading_accessory(mut self, element: impl IntoElement) -> Self {
        self.leading_accessory = Some(element.into_any_element());
        self
    }

    /// Set an optional leading accessory element.
    pub fn leading_accessory_opt(mut self, element: Option<AnyElement>) -> Self {
        self.leading_accessory = element;
        self
    }

    /// Set a trailing accessory element appended after the standard accessories area.
    ///
    /// This is a generic slot for domain-specific badges or indicators (e.g., "Saved"
    /// status badges in a theme chooser). The element is rendered as-is with flex_shrink_0.
    pub fn trailing_accessory(mut self, element: impl IntoElement) -> Self {
        self.trailing_accessory = Some(element.into_any_element());
        self
    }

    /// Set an optional trailing accessory element.
    pub fn trailing_accessory_opt(mut self, element: Option<AnyElement>) -> Self {
        self.trailing_accessory = element;
        self
    }
}
impl RenderOnce for ListItem {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let metrics = self
            .metrics_override
            .unwrap_or_else(|| ListItemMetricsOverride::from_main_menu_theme(self.main_menu_theme));
        let colors = self.colors;
        let index = self.index;
        let item_index = index.unwrap_or(0);
        let on_hover_callback = self.on_hover;
        let semantic_id = self.semantic_id;
        let shortcut_visibility_policy = self.shortcut_visibility_policy;
        let destructive_text_color = self.destructive_text_color;
        let disabled = self.disabled;

        // GPUI input modality: suppress hover visuals during keyboard navigation.
        // Disabled explanatory rows remain selectable but never brighten on hover.
        let hover_visible = self.hovered && !disabled && !window.last_input_was_keyboard();

        // Live main-menu theme exploration. Every variation shares the token-owned
        // leading selection marker, then coordinates row, icon, text, and metadata.
        use crate::designs::MainMenuRowKind;
        let row_kind = self.main_menu_theme.def().row_kind;
        // Pack the theme accent (0xRRGGBB) into an rgba color at the given alpha.
        let accent_at = |alpha: u32| rgba((colors.accent_selected << 8) | alpha);
        let selected = self.selected;
        let selection_marker_geometry = list_item_selection_marker_geometry(metrics, selected);
        let selection_marker_id = semantic_id
            .as_deref()
            .map(|id| format!("{id}:selection-marker"))
            .unwrap_or_else(|| format!("list-item:{item_index}:selection-marker"));

        let on_accent_text = matches!(
            row_kind,
            MainMenuRowKind::CarbonNeon | MainMenuRowKind::OceanGlass
        ) && selected
            && !disabled
            && matches!(row_kind, MainMenuRowKind::CarbonNeon);
        let icon_is_accent_bright = matches!(
            row_kind,
            MainMenuRowKind::BlueGlass
                | MainMenuRowKind::AuroraSlate
                | MainMenuRowKind::OceanGlass
                | MainMenuRowKind::CarbonNeon
                | MainMenuRowKind::OperatorMonoGlass
        ) && selected
            && !disabled;
        let icon_all_accent = matches!(row_kind, MainMenuRowKind::OperatorMonoGlass);
        let icon_tile = !matches!(
            row_kind,
            MainMenuRowKind::Smoke
                | MainMenuRowKind::GraphitePill
                | MainMenuRowKind::OperatorMonoGlass
        ) && selected
            && !disabled;
        let ring = matches!(
            row_kind,
            MainMenuRowKind::BlueGlass
                | MainMenuRowKind::LiquidPrism
                | MainMenuRowKind::AuroraSlate
                | MainMenuRowKind::ProConsole
                | MainMenuRowKind::OceanGlass
                | MainMenuRowKind::CarbonNeon
                | MainMenuRowKind::StudioPaperGlass
                | MainMenuRowKind::OperatorMonoGlass
        ) && selected
            && !disabled;
        let badges_accent = matches!(
            row_kind,
            MainMenuRowKind::FrostedCommand
                | MainMenuRowKind::AuroraSlate
                | MainMenuRowKind::OceanGlass
                | MainMenuRowKind::CarbonNeon
        );

        // Both hover and selected use text_primary at different opacities by
        // default. The byte resolution is shared with the design-contract
        // exporter — change resolved_main_menu_row_fill, not this call site.
        let row_fill = resolved_main_menu_row_fill(row_kind, &metrics, colors.hover_opacity);
        let row_states = crate::theme::resolve_main_menu_row_state_palette_from_parts(
            crate::theme::MainMenuRowColorInputs {
                row_kind,
                row_hover_fill_alpha: metrics.row_hover_fill_alpha as u8,
                row_selected_fill_alpha: metrics.row_selected_fill_alpha as u8,
                theme_hover_opacity: colors.hover_opacity,
                text_primary_hex: colors.text_primary,
                accent_selected_hex: colors.accent_selected,
                text_on_accent_hex: colors.text_on_accent,
                primary_name_alpha: colors.alpha_name as u8,
            },
        );
        let resolved_row_state = row_states.for_flags(crate::theme::RowStateFlags {
            selected,
            hovered: hover_visible,
            active: selected && !disabled,
            disabled,
        });
        let selected_bg: Hsla = rgba(if disabled {
            row_states
                .disabled_selected
                .background_rgba
                .expect("selected disabled main-menu rows always have a background")
        } else {
            row_states
                .active
                .background_rgba
                .expect("active main-menu rows always have a background")
        })
        .into();
        let hover_bg: Hsla = rgba(
            row_states
                .hover
                .background_rgba
                .expect("hovered main-menu rows always have a background"),
        )
        .into();

        // Icon element (if present) - displayed on the left
        // Supports both emoji strings and PNG image data
        let icon_text_color = if disabled {
            rgba(resolved_row_state.icon_foreground_rgba)
        } else if let Some(color) = destructive_text_color {
            rgb(color)
        } else if on_accent_text || icon_tile {
            // On a solid accent fill/tile, the glyph flips to the contrast color.
            rgb(colors.text_on_accent)
        } else if icon_is_accent_bright {
            accent_at(0xFF)
        } else if icon_all_accent {
            if selected {
                accent_at(0xFF)
            } else {
                accent_at(0xC0)
            }
        } else if selected {
            rgb(colors.text_primary)
        } else {
            rgba((colors.text_primary << 8) | colors.alpha_icon) // Quiet icons let names lead
        };
        let icon_size = px(metrics.icon_container_size);
        let svg_size = px(metrics.icon_svg_size);
        let icon_element = match &self.icon {
            Some(IconKind::Emoji(emoji)) => div()
                .w(icon_size)
                .h(icon_size)
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(icon_text_color)
                .flex_shrink_0()
                .child(emoji.clone()),
            Some(IconKind::Image(render_image)) => {
                // Render pre-decoded image directly (no decoding on render - critical for perf)
                let image = render_image.clone();
                div()
                    .w(icon_size)
                    .h(icon_size)
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .child(
                        img(move |_window: &mut Window, _cx: &mut App| Some(Ok(image.clone())))
                            .w(icon_size)
                            .h(icon_size)
                            .object_fit(ObjectFit::Contain),
                    )
            }
            Some(IconKind::Svg(name)) => {
                let svg_path = resolve_svg_icon_path(name);
                div()
                    .w(icon_size)
                    .h(icon_size)
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .child(
                        crate::utils::assets::svg_icon_source(svg(), &svg_path)
                            .size(svg_size)
                            .text_color(icon_text_color),
                    )
            }
            None => {
                div().w(px(0.)).h(px(0.)) // No space if no icon
            }
        };

        // Icon Tile: seat the selected icon inside a solid filled accent tile.
        // Alphas/radius resolve through the shared row-fill resolver so the
        // design-contract exporter stays byte-identical to this paint.
        let icon_tile_alpha = row_fill.icon_tile_alpha;
        let icon_tile_radius = row_fill.icon_tile_radius;
        let icon_element = if icon_tile && self.icon.is_some() {
            div()
                .w(px(metrics.icon_tile_size))
                .h(px(metrics.icon_tile_size))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(icon_tile_radius))
                .bg(accent_at(icon_tile_alpha))
                .flex_shrink_0()
                .child(icon_element)
        } else {
            icon_element
        };

        // Progressive disclosure: detect if search/filter is active
        // Used to conditionally show descriptions and accessories
        let is_filtering =
            self.highlight_indices.is_some() || self.description_highlight_indices.is_some();

        // Build content with name + description (compact with small gap)
        let mut item_content = div()
            .flex_1()
            .min_w(px(0.))
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap(px(metrics.name_desc_gap))
            .justify_center();

        // Name rendering - 14px font size for better balance with description
        // Medium weight for unselected, semibold when selected for clear emphasis
        // When highlight_indices are present, use StyledText to highlight matched characters
        // Otherwise, render as plain text
        let name_weight = if self.selected {
            metrics.selected_name_weight
        } else {
            metrics.name_weight
        };
        let name_element = if let Some(ref indices) = self.highlight_indices {
            // Build StyledText with highlighted matched characters.
            let highlight_color = if disabled {
                rgba(resolved_row_state.primary_foreground_rgba)
            } else if let Some(color) = destructive_text_color {
                rgb(color)
            } else if on_accent_text {
                rgb(colors.text_on_accent)
            } else {
                rgba(resolved_row_state.primary_foreground_rgba)
            };
            let highlight_style = HighlightStyle {
                color: Some(highlight_color.into()),
                ..Default::default()
            };

            // Convert character indices to byte ranges for StyledText
            let mut highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();
            for (char_idx, (byte_offset, ch)) in self.name.char_indices().enumerate() {
                if indices.contains(&char_idx) {
                    highlights.push((byte_offset..byte_offset + ch.len_utf8(), highlight_style));
                }
            }

            // Base text color is more muted when highlighting to create contrast
            let base_color = if disabled {
                rgba(resolved_row_state.secondary_foreground_rgba)
            } else if on_accent_text {
                rgba((colors.text_on_accent << 8) | 0xCC)
            } else if self.selected {
                rgba((colors.text_primary << 8) | colors.alpha_muted)
            } else {
                rgba((colors.text_primary << 8) | colors.alpha_hint)
            };

            let full_name = self.name.clone();
            let styled = StyledText::new(full_name.clone()).with_highlights(highlights);

            div()
                .text_size(px(metrics.name_font_size))
                .font_weight(name_weight)
                .overflow_hidden()
                .text_ellipsis()
                .id(ElementId::NamedInteger(
                    "list-name".into(),
                    item_index as u64,
                ))
                .whitespace_nowrap()
                .line_height(px(metrics.name_line_height))
                .text_color(base_color)
                .child(styled)
        } else {
            // Plain text rendering (no search active)
            let name_color = if disabled {
                rgba(resolved_row_state.primary_foreground_rgba)
            } else if let Some(color) = destructive_text_color {
                rgb(color)
            } else if on_accent_text {
                rgb(colors.text_on_accent)
            } else {
                rgba(resolved_row_state.primary_foreground_rgba)
            };
            div()
                .text_size(px(metrics.name_font_size))
                .font_weight(name_weight)
                .overflow_hidden()
                .text_ellipsis()
                .id(ElementId::NamedInteger(
                    "list-name".into(),
                    item_index as u64,
                ))
                .whitespace_nowrap()
                .line_height(px(metrics.name_line_height))
                .text_color(name_color)
                .child(self.name)
        };

        // Accent Name: optional underline beneath the bright-accent title.
        let name_underline_selected =
            selected && !disabled && metrics.row_selected_name_underline_width > 0.0;
        let name_element = if name_underline_selected {
            name_element
                .border_b(px(metrics.row_selected_name_underline_width))
                .border_color(accent_at(0xFF))
                .pb(px(metrics.row_selected_name_underline_padding_bottom))
        } else {
            name_element
        };
        item_content = item_content.child(name_element);

        // Description - progressive disclosure pattern (Spotlight/Raycast style)
        // Search mode keeps rows quieter by showing descriptions only when focused
        // or when the description itself contains a search match.
        if let Some(desc) = self.description {
            let has_description_match = self.description_highlight_indices.is_some();
            let show_description = if is_filtering {
                should_show_search_description(self.selected, hover_visible, has_description_match)
            } else {
                self.selected || hover_visible
            };

            if show_description {
                // Selected: use primary text (readable against selection bg)
                // Unselected: use secondary text (recedes in the list)
                // All descriptions use text_primary — opacity alone controls brightness
                let desc_color = if on_accent_text && !disabled {
                    rgba((colors.text_on_accent << 8) | 0xCC)
                } else {
                    rgba(resolved_row_state.secondary_foreground_rgba)
                };
                let desc_element = if let Some(ref desc_indices) =
                    self.description_highlight_indices
                {
                    // Build StyledText with highlighted matched characters in description.
                    let highlight_color = if on_accent_text && !disabled {
                        rgb(colors.text_on_accent)
                    } else {
                        rgba(resolved_row_state.primary_foreground_rgba)
                    };
                    let highlight_style = HighlightStyle {
                        color: Some(highlight_color.into()),
                        ..Default::default()
                    };

                    // Convert character indices to byte ranges for StyledText
                    let mut highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();
                    for (char_idx, (byte_offset, ch)) in desc.char_indices().enumerate() {
                        if desc_indices.contains(&char_idx) {
                            highlights
                                .push((byte_offset..byte_offset + ch.len_utf8(), highlight_style));
                        }
                    }

                    let base_color = rgba(resolved_row_state.secondary_foreground_rgba);
                    let full_desc = desc.clone();
                    let styled = StyledText::new(full_desc.clone()).with_highlights(highlights);

                    div()
                        .text_size(px(metrics.desc_font_size))
                        .line_height(px(metrics.desc_line_height))
                        .font_weight(metrics.desc_weight)
                        .text_color(base_color)
                        .overflow_hidden()
                        .text_ellipsis()
                        .id(ElementId::NamedInteger(
                            "list-desc".into(),
                            item_index as u64,
                        ))
                        .when(LIST_ITEM_MOUSE_HOVER_TOOLTIPS_ENABLED, |element| {
                            element.tooltip(move |window, cx| {
                                Tooltip::new(full_desc.clone()).build(window, cx)
                            })
                        })
                        .whitespace_nowrap()
                        .child(styled)
                } else {
                    let full_desc = desc.clone();
                    div()
                        .text_size(px(metrics.desc_font_size))
                        .line_height(px(metrics.desc_line_height))
                        .font_weight(metrics.desc_weight)
                        .text_color(desc_color)
                        .overflow_hidden()
                        .text_ellipsis()
                        .id(ElementId::NamedInteger(
                            "list-desc".into(),
                            item_index as u64,
                        ))
                        .when(LIST_ITEM_MOUSE_HOVER_TOOLTIPS_ENABLED, |element| {
                            element.tooltip(move |window, cx| {
                                Tooltip::new(full_desc.clone()).build(window, cx)
                            })
                        })
                        .whitespace_nowrap()
                        .child(desc)
                };
                item_content = item_content.child(desc_element);
            }
        }

        // Shortcut — compact inline glyphs via shared renderer (tokens cached at construction)
        let resolved_shortcut_tokens = list_item_shortcut_tokens_for_render(
            self.shortcut.as_deref(),
            self.shortcut_tokens.as_deref(),
        );
        let shortcut_element: AnyElement = if let Some(shortcut_tokens) =
            resolved_shortcut_tokens.as_ref()
        {
            let show_shortcut = !disabled
                && should_show_row_shortcut(
                    shortcut_visibility_policy,
                    self.selected,
                    hover_visible,
                );
            if show_shortcut {
                crate::components::hint_strip::emit_shortcut_chrome_audit(
                    "list_item",
                    "footer-keycap-selected-only",
                );
                let theme = crate::theme::get_cached_theme();
                crate::components::footer_chrome::render_footer_row_shortcut_keycaps_from_tokens(
                    shortcut_tokens.iter().map(String::as_str),
                    &theme,
                    resolved_row_state.primary_foreground_rgba,
                )
            } else {
                div().into_any_element()
            }
        } else {
            div().into_any_element()
        };

        // Determine background color based on selection/hover state
        // Priority: selected (full focus styling) > hovered (subtle feedback) > transparent
        // Note: For non-selected items, we ALSO apply GPUI's .hover() modifier for instant feedback
        let bg_color: Hsla = if self.selected {
            selected_bg // Theme-defined focused row highlight
        } else if hover_visible {
            hover_bg // Theme-defined hover feedback (state-based)
        } else {
            Hsla::transparent_black() // fully transparent
        };

        // Build the inner content div with all styling
        // Horizontal padding ITEM_PADDING_X and vertical padding ITEM_PADDING_Y
        //
        // HOVER TRANSITIONS: We use GPUI's built-in .hover() modifier for instant visual
        // feedback on non-selected items. This provides CSS-like instant hover effects
        // without waiting for state updates via cx.notify().
        //
        // For selected items, we don't apply hover styles (they already have full focus styling).
        // Rows use uniform leading padding regardless of selection.
        let pl_val = metrics.row_inner_padding_x;

        let inner_content_id = ElementId::NamedInteger("list-item-inner".into(), item_index as u64);
        let mut inner_content = div()
            .id(inner_content_id)
            .w_full()
            .h_full()
            .pl(px(pl_val))
            .pr(px(metrics.row_inner_padding_x))
            .py(px(metrics.row_inner_padding_y))
            .bg(bg_color)
            .rounded(px(metrics.row_radius))
            .text_color(rgb(colors.text_primary))
            .font_family(FONT_SYSTEM_UI)
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(metrics.icon_text_gap));

        // Only mount the icon slot when an icon exists: a zero-size placeholder
        // still consumes the row's flex gap, indenting iconless rows (e.g. the
        // actions popup) on the left while the right edge stays at the inner
        // padding — visually asymmetric padding.
        if self.icon.is_some() {
            inner_content = inner_content.child(icon_element);
        }

        // Leading accessory slot (e.g., color swatch strip) — between icon and text
        let has_leading_accessory = self.leading_accessory.is_some();
        let has_trailing_accessory = self.trailing_accessory.is_some();
        if let Some(leading) = self.leading_accessory {
            inner_content = inner_content.child(div().flex_shrink_0().child(leading));
        }

        let trailing_accessory = self.trailing_accessory;

        inner_content = inner_content.child(item_content).child({
            // Right-side accessories: [source hint] [type icon] [shortcut badge]
            let mut accessories = div()
                .flex()
                .flex_row()
                .items_center()
                .flex_shrink_0()
                .gap(px(metrics.accessory_gap));

            // Tool badge, source hint, and type accessory use progressive disclosure.
            // Search mode intentionally strips noisy metadata to keep rows calm.
            let show_accessories = self.selected || hover_visible || is_filtering;

            // Tool/language badge for scriptlets (e.g., "ts", "bash")
            if show_accessories && !is_filtering {
                if let Some(ref badge) = self.tool_badge {
                    // Loud lights up the metadata badges with the accent too.
                    let badge_is_accent = badges_accent;
                    let badge_bg = if badge_is_accent {
                        accent_at(0x40)
                    } else {
                        rgba((colors.text_primary << 8) | ALPHA_TINT_MEDIUM)
                    };
                    let badge_text = if badge_is_accent {
                        accent_at(0xFF)
                    } else {
                        rgba((colors.text_primary << 8) | colors.alpha_hint)
                    };
                    accessories = accessories.child(
                        div()
                            .text_size(px(metrics.badge_font_size))
                            .font_family(FONT_MONO)
                            .text_color(badge_text)
                            .px(px(metrics.badge_padding_x))
                            .py(px(metrics.badge_padding_y))
                            .rounded(px(metrics.badge_radius))
                            .bg(badge_bg)
                            .child(badge.clone()),
                    );
                }
            }

            // Source/kit hint (e.g., "main", "cleanshot")
            if show_accessories && !is_filtering {
                if let Some(ref hint) = self.source_hint {
                    accessories = accessories.child(
                        div()
                            .text_size(px(metrics.source_font_size))
                            .text_color(rgba((colors.text_primary << 8) | colors.alpha_hint))
                            .child(hint.clone()),
                    );
                }
            }

            // Type accessory stays visible during search as a quiet accent-tinted icon.
            if let Some(ref accessory) = self.type_accessory {
                let tooltip_label = accessory.label.to_string();
                let svg_path = resolve_svg_icon_path(accessory.icon_name);
                // Loud pushes the type accessory accent harder than the baseline.
                let accent_color = if badges_accent {
                    accent_at(if self.selected { 0xFF } else { 0x99 })
                } else {
                    rgba(row_type_accessory_rgba(&colors, self.selected))
                };
                accessories = accessories.child(
                    div()
                        .id(ElementId::Name(
                            format!("type-accessory-{}", accessory.label).into(),
                        ))
                        .when(LIST_ITEM_MOUSE_HOVER_TOOLTIPS_ENABLED, |element| {
                            element.tooltip(move |window, cx| {
                                Tooltip::new(tooltip_label.clone()).build(window, cx)
                            })
                        })
                        .flex_shrink_0()
                        .child(
                            crate::utils::assets::svg_icon_source(svg(), &svg_path)
                                .size(px(self.main_menu_theme.def().metadata.type_accessory_size))
                                .text_color(accent_color),
                        ),
                );
            }

            accessories = accessories.child(shortcut_element);
            accessories
        });

        // Trailing accessory slot (e.g., "Saved" status badge) — after standard accessories
        if let Some(trailing) = trailing_accessory {
            inner_content = inner_content.child(
                div()
                    .flex_shrink_0()
                    .ml(px(metrics.accessory_gap))
                    .child(trailing),
            );
        }

        // Emit chooser-row contract trace when accessory slots are used
        if has_leading_accessory || has_trailing_accessory {
            tracing::trace!(
                leading = has_leading_accessory,
                trailing = has_trailing_accessory,
                index = item_index,
                "list_item_accessory_contract"
            );
        }

        if !self.selected && !disabled {
            inner_content = inner_content.hover(move |s| s.bg(hover_bg));
        }

        // Use semantic_id for element ID if available, otherwise fall back to index
        // This allows AI agents to target elements by their semantic meaning
        let element_id = if let Some(ref sem_id) = semantic_id {
            // Use semantic ID as the element ID for better targeting
            ElementId::Name(sem_id.clone().into())
        } else {
            // Fall back to index-based ID
            ElementId::NamedInteger("list-item".into(), item_index as u64)
        };

        // Selection styling is governed by the main-menu theme explorer.

        // Accent Ring: outline the selected row with a thick accent border.
        if ring || metrics.row_selected_border_width > 0.0 {
            let border_width = metrics
                .row_selected_border_width
                .max(if ring { 2.0 } else { 0.0 });
            let border_alpha =
                metrics
                    .row_selected_border_alpha
                    .max(if ring { 0xE6 } else { 0x00 });
            inner_content = inner_content
                .border(px(border_width))
                .border_color(accent_at(border_alpha));
        }

        let selection_marker = selection_marker_geometry.map(|marker| {
            let debug_id = selection_marker_id.clone();
            div()
                .id(ElementId::Name(selection_marker_id.into()))
                .debug_selector(move || debug_id.clone())
                .absolute()
                .left(px(marker.left))
                .top(px(marker.top))
                .w(px(marker.width))
                .h(px(marker.height))
                .rounded(px(marker.radius))
                .bg(rgba((colors.accent_selected << 8) | marker.alpha))
        });

        // Base container with ID for stateful interactivity. `relative()` is
        // solely the positioning context for the absolute selection marker;
        // it does not alter row geometry, flex participation, or hit bounds.
        let mut container = div()
            .relative()
            .w_full()
            .h(px(metrics.item_height))
            .overflow_hidden()
            .px(px(metrics.row_outer_padding_x))
            .py(px(metrics.row_outer_padding_y))
            .flex()
            .flex_row()
            .items_center()
            .id(element_id);

        // Add hover handler if we have both index and callback
        if let (Some(idx), Some(callback)) = (index, on_hover_callback) {
            // Use Rc to allow sharing the callback in the closure
            let callback = std::rc::Rc::new(callback);

            container = container.on_hover(move |hovered: &bool, _window, _cx| {
                // Log the mouse enter/leave event
                if *hovered {
                    logging::log_mouse_enter(idx, None);
                } else {
                    logging::log_mouse_leave(idx, None);
                }
                // Call the user-provided callback
                callback(idx, *hovered);
            });
        }

        // The selected row is where the app's attention is: report its
        // painted position so the background shader effect can lean toward
        // it (main-window only; the probe is a zero-cost absolute overlay).
        if self.selected {
            container = container.child(crate::effects::effect_focus_probe(
                crate::effects::EffectFocusSource::FocusedItem,
            ));
        }

        // Add content first, then paint the absolute marker above the selected
        // surface without participating in flex or changing the row hit box.
        container = container.child(inner_content);
        if let Some(selection_marker) = selection_marker {
            container = container.child(selection_marker);
        }
        container
    }
}
/// Decode PNG bytes to GPUI RenderImage
///
/// Decode PNG bytes to a GPUI RenderImage
///
/// Uses the `image` crate to decode PNG data and creates a GPUI-compatible
/// RenderImage for display. Returns an Arc<RenderImage> for caching.
///
/// **IMPORTANT**: Call this ONCE when loading icons, NOT during rendering.
/// Decoding PNGs on every render frame causes severe performance issues.
pub fn decode_png_to_render_image(png_data: &[u8]) -> Result<Arc<RenderImage>, image::ImageError> {
    decode_png_to_render_image_internal(png_data, false)
}
/// Decode PNG bytes to GPUI RenderImage with RGBA→BGRA conversion for Metal
///
/// GPUI/Metal expects BGRA pixel format. When creating RenderImage directly
/// from image::Frame (bypassing GPUI's internal loaders), we must do the
/// RGBA→BGRA conversion ourselves. This matches what GPUI does internally
/// in platform.rs for loaded images.
///
/// **IMPORTANT**: Call this ONCE when loading icons, NOT during rendering.
pub fn decode_png_to_render_image_with_bgra_conversion(
    png_data: &[u8],
) -> Result<Arc<RenderImage>, image::ImageError> {
    decode_png_to_render_image_internal(png_data, true)
}
fn decode_png_to_render_image_internal(
    png_data: &[u8],
    convert_to_bgra: bool,
) -> Result<Arc<RenderImage>, image::ImageError> {
    use image::GenericImageView;
    use smallvec::SmallVec;

    // Decode PNG
    let img = image::load_from_memory(png_data)?;

    // Convert to RGBA8
    let mut rgba = img.to_rgba8();
    let (width, height) = img.dimensions();

    // Convert RGBA to BGRA for Metal/GPUI rendering
    // GPUI's internal image loading does this swap (see gpui/src/platform.rs)
    // We must do the same when creating RenderImage directly from image::Frame
    if convert_to_bgra {
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2); // Swap R and B: RGBA -> BGRA
        }
    }

    // Create Frame from buffer (now in BGRA order if converted)
    let buffer = image::RgbaImage::from_raw(width, height, rgba.into_raw()).ok_or_else(|| {
        image::ImageError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Failed to create image buffer",
        ))
    })?;
    let frame = image::Frame::new(buffer);

    // Create RenderImage
    let render_image = RenderImage::new(SmallVec::from_elem(frame, 1));

    Ok(Arc::new(render_image))
}
/// Create an IconKind from PNG bytes by pre-decoding them
///
/// Returns None if decoding fails. This should be called once when loading
/// icons, not during rendering.
pub fn icon_from_png(png_data: &[u8]) -> Option<IconKind> {
    decode_png_to_render_image(png_data)
        .ok()
        .map(IconKind::Image)
}
/// Render a section header for grouped lists (e.g., "Recent", "Main")
///
/// Visual design for section headers:
/// - Uppercase whisper label for stronger scanability
/// - 12px font (meets desktop minimum)
/// - Normal weight so headers recede behind items
/// - Dimmed color (subtle but readable)
/// - 32px height (8px grid aligned)
/// - Left-aligned with list item padding
/// - No separators or background tint; spacing alone defines groups
///
/// ## Technical Note: list() Height
/// Uses GPUI's `list()` component which supports variable-height items.
/// Section headers render at 32px, regular items at 40px.
///
/// # Arguments
/// * `label` - Authored section text; launcher presentation uppercases a derived display copy
/// * `icon` - Optional icon name (lucide icon, e.g., "settings")
/// * `colors` - ListItemColors for theme-aware styling
/// * `_is_first` - Reserved for existing call sites; unused because headers no longer draw separators
///
/// Section headers are single-line by contract: exactly one separator row per
/// group, matching the main-menu list language everywhere (launcher, Agent
/// Chat composer pickers, notes spine). A two-line title+subtitle header reads
/// as a doubled separator (2026-07-10 "@" context picker bug), so no subtitle
/// variant exists — section subtitles stay data-only (tooltips/protocol), not
/// list chrome.
pub fn render_section_header(
    label: &str,
    icon: Option<&str>,
    colors: ListItemColors,
    is_first: bool,
) -> impl IntoElement {
    let metrics = resolved_list_item_metrics();
    // Section header at 32px (8px grid aligned, SECTION_HEADER_HEIGHT)
    // Used with GPUI's list() component which supports variable-height items.
    //
    // Layout: 32px total height
    // - pt(12px) top padding for visual separation from above item
    // - ~12px text height
    // - pb(4px) bottom padding for visual separation from below item

    // Parse count metadata without mutating the authored semantic label.
    let (section_name, count_text) = if let Some(dot_pos) = label.find(" · ") {
        (&label[..dot_pos], Some(&label[dot_pos + " · ".len()..]))
    } else {
        (label, None)
    };
    let presentation = resolve_section_header_presentation(
        section_name,
        icon,
        count_text,
        SectionPresentationFamily::Launcher,
    );
    let tier_alpha = |tier| match tier {
        SectionTextTier::Strong => colors.alpha_strong,
        SectionTextTier::Muted => colors.alpha_muted,
    };

    // Launcher labels lead strongly; metadata remains quiet and never inherits label emphasis.
    let mut content = div()
        .flex()
        .flex_row()
        .items_center()
        .min_w(px(0.0))
        .gap(px(metrics.section_gap))
        .text_size(px(metrics.section_header_font_size))
        .font_weight(metrics.section_weight);

    if let Some(name) = presentation.icon.as_deref() {
        if let Some(icon_name) = icon_name_from_str(name) {
            content = content.child(
                svg()
                    .path(icon_name.asset_path())
                    .size(px(metrics.section_icon_size))
                    .text_color(rgba(
                        (colors.text_primary << 8) | tier_alpha(presentation.icon_tier),
                    )),
            );
        }
    }

    content = content.child(
        div()
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .text_color(rgba(
                (colors.text_primary << 8) | tier_alpha(presentation.label_tier),
            ))
            .child(presentation.display_label.clone()),
    );

    if let Some(count) = presentation.count {
        content = content.child(
            div()
                .flex_none()
                .text_xs()
                .font_weight(metrics.section_weight)
                .text_color(rgba(
                    (colors.text_primary << 8) | tier_alpha(presentation.count_tier),
                ))
                .child(count),
        );
    }

    let (header_height, padding_top) = if is_first {
        // First section headers align their top breathing room with the shared
        // main-view header bottom padding, so the query/header block and the
        // first group separator feel mathematically balanced.
        (
            metrics.first_section_header_height,
            metrics.first_section_padding_top,
        )
    } else {
        (metrics.section_header_height, metrics.section_padding_top)
    };

    // Clean section headers — no background tint for a calmer list appearance
    let mut header = div()
        .w_full()
        .h(px(header_height))
        .px(px(metrics.section_padding_x))
        .pt(px(padding_top))
        .pb(px(metrics.section_padding_bottom))
        .flex()
        .flex_col();
    header = if is_first {
        header.justify_start()
    } else {
        header.justify_end()
    }; // Later headers stay bottom-anchored between rows.

    // No separator lines — spacing alone defines groups per whisper-chrome spec
    header.child(content)
}
// Note: GPUI rendering tests omitted due to GPUI macro recursion limit issues.
// The LIST_ITEM_HEIGHT constant is 40.0 and the component is integration-tested
// via the main application's script list and arg prompt rendering.
// Unit tests for format_shortcut_display are in src/list_item_tests.rs.

#[cfg(test)]
mod render_section_header_source_tests {
    use super::{
        ensure_launcher_section_slot, grouped_list_item_eligibility,
        resolve_section_header_presentation, GroupedListItem, SectionPresentationFamily,
        SectionTextTier,
    };

    const SOURCE: &str = include_str!("mod.rs");

    fn render_section_header_source() -> String {
        let start = SOURCE
            .find("pub fn render_section_header(")
            .expect("render_section_header should exist");
        let rest = &SOURCE[start..];
        let end = rest
            .find("// Note: GPUI rendering tests omitted")
            .expect("sentinel comment should exist after render_section_header");
        rest[..end].to_string()
    }

    #[test]
    fn section_header_presentation_preserves_semantics_and_transforms_display_only() {
        let authored = "Straße · résumé";
        let launcher = resolve_section_header_presentation(
            authored,
            Some("star"),
            Some("5"),
            SectionPresentationFamily::Launcher,
        );
        assert_eq!(launcher.semantic_label.as_ref(), authored);
        assert_eq!(launcher.display_label.as_ref(), "STRASSE · RÉSUMÉ");
        assert_eq!(launcher.label_tier, SectionTextTier::Strong);
        assert_eq!(launcher.count_tier, SectionTextTier::Muted);
        assert_eq!(launcher.icon_tier, SectionTextTier::Muted);

        let preserved = resolve_section_header_presentation(
            authored,
            None,
            None,
            SectionPresentationFamily::PreserveAuthored,
        );
        assert_eq!(preserved.semantic_label.as_ref(), authored);
        assert_eq!(preserved.display_label.as_ref(), authored);
        assert_eq!(preserved.label_tier, SectionTextTier::Muted);
    }

    #[test]
    fn launcher_section_slot_is_stable_inert_and_non_semantic() {
        let mut ungrouped = vec![GroupedListItem::Item(0)];
        ensure_launcher_section_slot(&mut ungrouped);
        assert!(matches!(
            ungrouped.as_slice(),
            [
                GroupedListItem::ReservedSectionSlot,
                GroupedListItem::Item(0)
            ]
        ));
        let reserved = grouped_list_item_eligibility(&ungrouped[0]);
        assert!(!reserved.focusable && !reserved.selectable && !reserved.activatable);

        let mut grouped = vec![
            GroupedListItem::SectionHeader("Recent".into(), None),
            GroupedListItem::Item(0),
        ];
        ensure_launcher_section_slot(&mut grouped);
        assert_eq!(grouped.len(), 2, "a real first header must keep the slot");

        let mut empty = Vec::new();
        ensure_launcher_section_slot(&mut empty);
        assert!(
            empty.is_empty(),
            "zero results must not expose an empty slot"
        );
    }

    #[test]
    fn section_header_geometry_comes_from_metrics() {
        let body = render_section_header_source();
        for required in [
            "metrics.section_padding_x",
            "metrics.section_padding_bottom",
            "metrics.section_gap",
            "metrics.section_icon_size",
            "metrics.section_weight",
        ] {
            assert!(
                body.contains(required),
                "render_section_header should use {required}"
            );
        }
        for forbidden in [
            "SECTION_PADDING_X",
            "SECTION_PADDING_BOTTOM",
            "SECTION_GAP",
            "SECTION_HEADER_ICON_SIZE",
            "FontWeight::SEMIBOLD",
        ] {
            assert!(
                !body.contains(forbidden),
                "render_section_header should not use local {forbidden}"
            );
        }
    }

    #[test]
    fn section_headers_do_not_render_separator_lines() {
        let body = render_section_header_source();
        assert!(
            !body.contains("border_t_1"),
            "section headers should rely on spacing, not separator lines"
        );
    }

    #[test]
    fn section_header_docs_do_not_reference_removed_top_border_behavior() {
        let body = render_section_header_source();
        assert!(
            !body.contains("suppresses top border"),
            "render_section_header docs should not describe removed separator behavior"
        );
    }
}

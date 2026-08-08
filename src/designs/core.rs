#[allow(unused_imports)]
use super::*;

mod accent_variation;
mod actions_popup_theme;
mod main_menu_theme;
mod metadata;
pub mod registry;
mod render;
mod tokens;
mod variant;

#[cfg(test)]
mod match_reason;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use accent_variation::{
    current_accent_variation, set_current_accent_variation, AccentVariation,
};
#[allow(unused_imports)]
pub use main_menu_theme::{
    current_main_menu_theme, set_current_main_menu_theme, FooterButtonTheme, FooterMetricsTokens,
    FooterTheme, HeaderInfoBarLayout, HeaderInfoBarTokens, MainMenuGeometrySignature,
    MainMenuIconTokens, MainMenuInputTextAlignment, MainMenuListTokens, MainMenuLogoPlacement,
    MainMenuMetadataTokens, MainMenuRowKind, MainMenuRowTokens, MainMenuSearchTokens,
    MainMenuSectionMetrics, MainMenuShellTokens, MainMenuThemeDef, MainMenuThemeTier,
    MainMenuThemeVariant, MainMenuTypographyTokens, MAIN_MENU_HEADER_CONTEXT_EDGE_OUTSET_X,
    MAIN_MENU_METADATA_BADGE_FONT_SIZE, MAIN_MENU_METADATA_BADGE_PADDING_X,
    MAIN_MENU_METADATA_BADGE_PADDING_Y, MAIN_MENU_METADATA_BADGE_RADIUS,
    MAIN_MENU_METADATA_SOURCE_FONT_SIZE, MAIN_MENU_SECTION_GAP, MAIN_MENU_SECTION_ICON_SIZE,
    MAIN_MENU_SECTION_PADDING_BOTTOM, MAIN_MENU_SECTION_PADDING_TOP, MAIN_MENU_SECTION_PADDING_X,
    MAIN_MENU_SECTION_WEIGHT,
};
// `FooterButtonFill` is part of the accent-variation API surface; re-exported for
// callers that name the type even though the footer consumes it field-by-field.
#[allow(unused_imports)]
pub use accent_variation::FooterButtonFill;
#[allow(unused_imports)]
pub use actions_popup_theme::{
    base_actions_popup_theme, current_actions_popup_theme, ActionsPopupContextHeaderTokens,
    ActionsPopupListTokens, ActionsPopupRowAlphaBytes, ActionsPopupRowTokens,
    ActionsPopupSearchTokens, ActionsPopupSectionTokens, ActionsPopupShellTokens,
    ActionsPopupThemeDef,
};
pub use render::render_design_item;
pub use tokens::*;
pub use variant::*;

#[cfg(test)]
pub(crate) use match_reason::*;
#[cfg(test)]
pub(crate) use metadata::*;
#[cfg(test)]
pub(crate) use render::{
    extension_default_icon, resolve_search_accessories, resolve_tool_badge, root_file_type_svg_icon,
};

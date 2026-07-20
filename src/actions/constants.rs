//! Actions dialog constants
//!
//! Overlay popup dimensions and styling constants used by the ActionsDialog.

/// Popup width for the actions dialog
pub const POPUP_WIDTH: f32 = 340.0;

/// Maximum height for the actions dialog popup
pub const POPUP_MAX_HEIGHT: f32 = 400.0;

/// Taller popup cap for the Notes Cmd+P recent-note switcher.
pub const NOTES_RECENT_POPUP_MAX_HEIGHT: f32 = 560.0;

/// Canonical compact row height for action popups.
///
/// Intentionally SHORTER than the main-list `LIST_ITEM_HEIGHT` (40): action
/// popups are a compact, title-only surface. The actions popup theme
/// (`designs/core/actions_popup_theme.rs`) derives `list.row_height` from
/// this token. Switcher-style hosts that opt into subtitles
/// (`ActionsDialogConfig::show_subtitles`) use main-list row anatomy via
/// `ActionsDialog::effective_row_height` instead.
pub const ACTION_ITEM_HEIGHT: f32 = 36.0;

/// Fixed height for the search input row
pub const SEARCH_INPUT_HEIGHT: f32 = 40.0;

/// Height for the header row showing context title (matches section header style)
pub const HEADER_HEIGHT: f32 = 26.0;

/// Horizontal padding for section/header rows in the actions dialog
pub const ACTION_PADDING_X: f32 = 12.0;

/// Top padding for section/header rows in the actions dialog
pub const ACTION_PADDING_TOP: f32 = 8.0;

/// Horizontal inset for action rows (creates rounded pill appearance)
pub const ACTION_ROW_INSET: f32 = 8.0;

/// Outer and inner radius for the Tahoe-style actions popup shell.
/// De-drifted onto the shared Liquid Glass popup/compact radius tokens.
pub const ACTIONS_POPUP_RADIUS: f32 = crate::ui::chrome::LIQUID_GLASS_POPUP_RADIUS_PX;
pub const ACTIONS_ROW_RADIUS: f32 = crate::ui::chrome::LIQUID_GLASS_COMPACT_RADIUS_PX;

/// `.impeccable.md` contract: actions dialog groups are spacing-defined headers.
pub const ACTIONS_DIALOG_EXPECT_SECTION_MODE: &str = "headers";

/// `.impeccable.md` contract: the search input stays bare, so no divider.
pub const ACTIONS_DIALOG_EXPECT_SEARCH_DIVIDER: bool = false;

/// `.impeccable.md` contract: actions dialogs do not render footer hints.
pub const ACTIONS_DIALOG_EXPECT_FOOTER_HINT_COUNT: u8 = 0;

/// `.impeccable.md` contract: search input belongs at the top of the dialog.
pub const ACTIONS_DIALOG_EXPECT_SEARCH_POSITION: &str = "top";

/// `.impeccable.md` contract: no visible container border (whisper chrome).
pub const ACTIONS_DIALOG_EXPECT_CONTAINER_BORDER: bool = false;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popup_constants() {
        assert_eq!(POPUP_WIDTH, 340.0);
        assert_eq!(POPUP_MAX_HEIGHT, 400.0);
        assert_eq!(NOTES_RECENT_POPUP_MAX_HEIGHT, 560.0);
        assert!(NOTES_RECENT_POPUP_MAX_HEIGHT > POPUP_MAX_HEIGHT);
    }

    #[test]
    fn test_action_item_height_constant() {
        // Popup rows are deliberately more compact than main-list rows;
        // see the ACTION_ITEM_HEIGHT doc comment for the contract.
        assert_eq!(ACTION_ITEM_HEIGHT, 36.0);
        assert!(ACTION_ITEM_HEIGHT < crate::list_item::LIST_ITEM_HEIGHT);
        const _: () = assert!(ACTION_ITEM_HEIGHT > 0.0);
        const _: () = assert!(ACTION_ITEM_HEIGHT < POPUP_MAX_HEIGHT);
    }

    #[test]
    fn test_max_visible_items() {
        let max_visible = (POPUP_MAX_HEIGHT / ACTION_ITEM_HEIGHT) as usize;
        assert!(max_visible >= 8, "Should fit at least 8 items");
        assert!(max_visible <= 15, "Sanity check on max visible");
    }

    #[test]
    fn test_action_padding_constants() {
        assert_eq!(ACTION_PADDING_X, 12.0);
        assert_eq!(ACTION_PADDING_TOP, 8.0);
    }

    #[test]
    fn test_actions_dialog_contract_constants_match_impeccable() {
        assert_eq!(ACTIONS_DIALOG_EXPECT_SECTION_MODE, "headers");
        let divider = ACTIONS_DIALOG_EXPECT_SEARCH_DIVIDER;
        assert!(!divider);
        assert_eq!(ACTIONS_DIALOG_EXPECT_FOOTER_HINT_COUNT, 0);
    }

    #[test]
    fn actions_dialog_contract_constants_match_impeccable_spec() {
        assert_eq!(ACTIONS_DIALOG_EXPECT_SEARCH_POSITION, "top");
        // Use a let-binding to avoid clippy::assertions_on_constants
        let border = ACTIONS_DIALOG_EXPECT_CONTAINER_BORDER;
        assert!(!border);
    }
}

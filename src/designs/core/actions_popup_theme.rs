use gpui::FontWeight;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionsPopupThemeDef {
    pub shell: ActionsPopupShellTokens,
    pub search: ActionsPopupSearchTokens,
    pub list: ActionsPopupListTokens,
    pub row: ActionsPopupRowTokens,
    pub section: ActionsPopupSectionTokens,
    pub context_header: ActionsPopupContextHeaderTokens,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionsPopupShellTokens {
    pub width: f32,
    pub max_height: f32,
    pub margin_x: f32,
    pub margin_y: f32,
    pub titlebar_offset_y: f32,
    pub border_height: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionsPopupSearchTokens {
    pub height: f32,
    pub inner_height: f32,
    pub padding_x: f32,
    pub padding_y_extra: f32,
    pub font_size: f32,
    pub cursor_width: f32,
    pub cursor_height: f32,
    pub prefix_gap: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionsPopupListTokens {
    pub row_height: f32,
    pub empty_row_height: f32,
    pub section_header_height: f32,
    pub padding_top: f32,
    pub padding_bottom: f32,
    pub overdraw_px: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionsPopupRowTokens {
    pub inset_x: f32,
    pub radius: f32,
    pub selection_opacity: f32,
    pub hover_opacity: f32,
    pub title_font_size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionsPopupSectionTokens {
    pub padding_x: f32,
    pub padding_top: f32,
    pub padding_bottom: f32,
    pub font_size: f32,
    pub font_weight: FontWeight,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionsPopupContextHeaderTokens {
    pub height: f32,
    pub padding_x: f32,
    pub padding_top: f32,
    pub padding_bottom: f32,
    pub font_size: f32,
    pub font_weight: FontWeight,
}

pub fn base_actions_popup_theme() -> ActionsPopupThemeDef {
    ActionsPopupThemeDef {
        shell: ActionsPopupShellTokens {
            width: crate::actions::constants::POPUP_WIDTH,
            max_height: crate::actions::constants::POPUP_MAX_HEIGHT,
            margin_x: 8.0,
            margin_y: 8.0,
            titlebar_offset_y: 36.0,
            border_height: 2.0,
            radius: crate::actions::constants::ACTIONS_POPUP_RADIUS,
        },
        search: ActionsPopupSearchTokens {
            height: 40.0,
            inner_height: 28.0,
            padding_x: crate::actions::constants::ACTION_PADDING_X,
            padding_y_extra: 2.0,
            // Actions popups are a compact satellite surface: the search/header
            // text stays smaller than the main-menu search (20) but matches
            // main-list names (14) for readability.
            font_size: 14.0,
            cursor_width: 2.0,
            cursor_height: 14.0,
            prefix_gap: 6.0,
        },
        list: ActionsPopupListTokens {
            row_height: crate::actions::constants::ACTION_ITEM_HEIGHT,
            empty_row_height: crate::actions::constants::ACTION_ITEM_HEIGHT,
            section_header_height: 24.0,
            padding_top: 0.0,
            // Breathing room below the last row so it doesn't sit flush
            // against the popup's bottom edge. Flows through window sizing
            // (actions_window_dynamic_height) and scrollbar viewport math.
            padding_bottom: 6.0,
            overdraw_px: 100.0,
        },
        row: ActionsPopupRowTokens {
            inset_x: crate::actions::constants::ACTION_ROW_INSET,
            radius: crate::actions::constants::ACTIONS_ROW_RADIUS,
            selection_opacity: 0.72,
            hover_opacity: 0.56,
            // Matches the main-list name font (14): 13 read as too small in
            // practice even though action rows are one-line commands.
            title_font_size: 14.0,
        },
        section: ActionsPopupSectionTokens {
            padding_x: crate::actions::constants::ACTION_PADDING_X,
            padding_top: crate::actions::constants::ACTION_PADDING_TOP,
            padding_bottom: 4.0,
            font_size: 12.0,
            font_weight: FontWeight::SEMIBOLD,
        },
        context_header: ActionsPopupContextHeaderTokens {
            height: crate::actions::constants::HEADER_HEIGHT,
            padding_x: crate::actions::constants::ACTION_PADDING_X,
            padding_top: crate::actions::constants::ACTION_PADDING_TOP,
            padding_bottom: 4.0,
            font_size: 12.0,
            font_weight: FontWeight::SEMIBOLD,
        },
    }
}

pub fn current_actions_popup_theme() -> ActionsPopupThemeDef {
    let mut def = base_actions_popup_theme();
    def.search.cursor_height = def.search.font_size;
    def
}

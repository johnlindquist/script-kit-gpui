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

impl ActionsPopupSearchTokens {
    /// Renderer-derived cursor height; not an independently writable token.
    pub fn resolved_cursor_height(self) -> f32 {
        self.font_size
    }
}

/// Typed authored-byte projection of the Actions row alphas (GOV-003), for
/// the design-contract serializer. The renderer-facing fields stay
/// normalized `f32` (GPUI consumes normalized opacity); quantization to the
/// byte domain happens HERE, once, through the canonical truncating
/// constructor — there is no pre-existing Actions byte quantization to
/// preserve, so the canonical `from_normalized` algorithm is adopted and
/// recorded in the GOV-003 receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionsPopupRowAlphaBytes {
    pub selection: crate::theme::AlphaByte,
    pub hover: crate::theme::AlphaByte,
}

impl ActionsPopupRowTokens {
    pub fn alpha_bytes(self) -> ActionsPopupRowAlphaBytes {
        ActionsPopupRowAlphaBytes {
            selection: crate::theme::AlphaByte::from_normalized(self.selection_opacity),
            hover: crate::theme::AlphaByte::from_normalized(self.hover_opacity),
        }
    }
}

pub fn current_actions_popup_theme() -> ActionsPopupThemeDef {
    base_actions_popup_theme()
}

#[cfg(test)]
mod actions_popup_theme_tests {
    use super::*;

    /// The derived relation production paints: cursor height == font size.
    #[test]
    fn cursor_height_is_derived_from_the_search_font_size() {
        let current = current_actions_popup_theme();
        assert_eq!(
            current.search.resolved_cursor_height(),
            current.search.font_size
        );
        // Changing the source font updates the derived metric.
        let mut base = base_actions_popup_theme();
        base.search.font_size = 16.0;
        assert_eq!(base.search.resolved_cursor_height(), 16.0);
    }

    /// Exact quantization lock for the retained normalized alphas: the
    /// canonical TRUNCATING constructor, at today's authored values.
    /// 0.72 × 255 = 183.6 → 0xB7; 0.56 × 255 = 142.8 → 0x8E.
    #[test]
    fn row_alpha_bytes_quantize_with_the_canonical_truncating_algorithm() {
        let row = base_actions_popup_theme().row;
        assert_eq!(row.selection_opacity, 0.72);
        assert_eq!(row.hover_opacity, 0.56);
        let bytes = row.alpha_bytes();
        assert_eq!(bytes.selection.get(), 0xB7);
        assert_eq!(bytes.hover.get(), 0x8E);
        // Negative control: rounding would disagree by one byte on
        // selection — the algorithms must not be silently swapped.
        assert_ne!(
            crate::theme::AlphaByte::from_normalized_rounded(0.72).get(),
            bytes.selection.get()
        );
    }

    /// Retained Actions defaults hold their pre-change values (GEO-008 is
    /// not permission to change Actions layout/density).
    #[test]
    fn retained_token_defaults_hold() {
        let def = base_actions_popup_theme();
        assert_eq!(def.search.font_size, 14.0);
        assert_eq!(def.search.cursor_width, 2.0);
        assert_eq!(def.search.resolved_cursor_height(), 14.0);
        assert_eq!(def.list.padding_bottom, 6.0);
        assert_eq!(def.list.overdraw_px, 100.0);
        assert_eq!(
            def.section.padding_top,
            crate::actions::constants::ACTION_PADDING_TOP
        );
        assert_eq!(def.section.padding_bottom, 4.0);
        assert_eq!(def.row.title_font_size, 14.0);
    }
}

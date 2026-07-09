// Day Page vertical layout budget shared by rendering and DevTools receipts.

pub(crate) const DAY_PAGE_MIN_EDITOR_HEIGHT_PX: f32 = 180.0;
pub(crate) const DAY_PAGE_CLIPBOARD_SHELF_TOP_PADDING_PX: f32 = 6.0;
pub(crate) const DAY_PAGE_CLIPBOARD_SHELF_TOGGLE_HEIGHT_PX: f32 = 20.0;
pub(crate) const DAY_PAGE_CLIPBOARD_SHELF_GAP_PX: f32 = 4.0;
pub(crate) const DAY_PAGE_CLIPBOARD_SHELF_ROW_HEIGHT_PX: f32 = 24.0;
const DAY_PAGE_CLIPBOARD_SHELF_MAX_BODY_FRACTION: f32 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DayPageLayoutBudget {
    pub(crate) body_height: f32,
    pub(crate) editor_height: f32,
    pub(crate) shelf_height: f32,
    pub(crate) shelf_list_height: f32,
}

/// One vertical owner for the Day Page editor and its clipboard accessory.
/// Rendering and DevTools receipts both consume this calculation.
pub(crate) fn day_page_layout_budget(
    viewport_height: f32,
    header_height: f32,
    footer_height: f32,
    shelf_count: usize,
    shelf_expanded: bool,
    accessory_bottom_padding: f32,
) -> DayPageLayoutBudget {
    let body_height = (viewport_height - header_height - footer_height).max(0.0);
    if shelf_count == 0 {
        return DayPageLayoutBudget {
            body_height,
            editor_height: body_height,
            shelf_height: 0.0,
            shelf_list_height: 0.0,
        };
    }

    let shelf_chrome_height = DAY_PAGE_CLIPBOARD_SHELF_TOP_PADDING_PX
        + DAY_PAGE_CLIPBOARD_SHELF_TOGGLE_HEIGHT_PX
        + accessory_bottom_padding;
    let expanded_list_gap = if shelf_expanded {
        DAY_PAGE_CLIPBOARD_SHELF_GAP_PX
    } else {
        0.0
    };
    let available_after_min_editor =
        (body_height - DAY_PAGE_MIN_EDITOR_HEIGHT_PX - shelf_chrome_height - expanded_list_gap)
            .max(0.0);
    let responsive_list_cap =
        available_after_min_editor.min(body_height * DAY_PAGE_CLIPBOARD_SHELF_MAX_BODY_FRACTION);
    let desired_list_height = shelf_count as f32 * DAY_PAGE_CLIPBOARD_SHELF_ROW_HEIGHT_PX;
    let shelf_list_height = if shelf_expanded {
        desired_list_height.min(responsive_list_cap)
    } else {
        0.0
    };
    let shelf_height = shelf_chrome_height
        + shelf_list_height
        + if shelf_list_height > 0.0 {
            expanded_list_gap
        } else {
            0.0
        };

    DayPageLayoutBudget {
        body_height,
        editor_height: (body_height - shelf_height).max(0.0),
        shelf_height,
        shelf_list_height,
    }
}

#[cfg(test)]
mod day_page_layout_budget_tests {
    use super::*;

    #[test]
    fn expanded_shelf_preserves_editor_minimum_at_compact_height() {
        let budget = day_page_layout_budget(360.0, 68.0, 36.0, 20, true, 12.0);

        assert_eq!(budget.body_height, 256.0);
        assert_eq!(budget.editor_height, DAY_PAGE_MIN_EDITOR_HEIGHT_PX);
        assert_eq!(budget.shelf_list_height, 34.0);
        assert_eq!(
            budget.editor_height + budget.shelf_height,
            budget.body_height
        );
    }

    #[test]
    fn shelf_list_budget_responds_to_available_height() {
        let compact = day_page_layout_budget(360.0, 68.0, 36.0, 20, true, 12.0);
        let tall = day_page_layout_budget(640.0, 68.0, 36.0, 20, true, 12.0);

        assert!(compact.shelf_list_height < tall.shelf_list_height);
        assert_ne!(compact.shelf_list_height, 180.0);
        assert!(tall.editor_height >= DAY_PAGE_MIN_EDITOR_HEIGHT_PX);
    }

    #[test]
    fn collapsed_or_absent_shelf_consumes_no_list_budget() {
        let collapsed = day_page_layout_budget(480.0, 68.0, 36.0, 4, false, 12.0);
        let absent = day_page_layout_budget(480.0, 68.0, 36.0, 0, true, 12.0);

        assert_eq!(collapsed.shelf_list_height, 0.0);
        assert_eq!(collapsed.shelf_height, 38.0);
        assert_eq!(absent.shelf_height, 0.0);
        assert_eq!(absent.editor_height, absent.body_height);
    }
}

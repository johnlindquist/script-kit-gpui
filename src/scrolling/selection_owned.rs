use crate::list_item::{coerce_selection, GroupedListItem};
use gpui::{ListOffset, Pixels};

#[allow(dead_code)] // Shared with the launcher binary; the library target compiles this module without the owning call sites.
pub(crate) fn visible_grouped_row_range(
    rows: &[GroupedListItem],
    scroll_top: ListOffset,
    viewport_height: Pixels,
) -> Option<(usize, usize)> {
    let theme = crate::designs::current_main_menu_theme();
    let heights = crate::scrolling::list_geometry::GroupedListRowHeights::for_theme(theme);
    crate::scrolling::list_geometry::visible_range_for_offset(
        rows.len(),
        scroll_top,
        viewport_height,
        |ix| heights.row_height(&rows[ix], ix),
    )
    .map(|(first, last_exclusive)| (first, last_exclusive - 1))
}

#[allow(dead_code)] // Shared with the launcher binary; the library target compiles this module without the owning call sites.
pub(crate) fn reanchor_grouped_selection(
    rows: &[GroupedListItem],
    current_selected: usize,
    scroll_top: ListOffset,
    viewport_height: Pixels,
) -> Option<usize> {
    let (first, last) = visible_grouped_row_range(rows, scroll_top, viewport_height)?;
    if current_selected >= first
        && current_selected <= last
        && matches!(rows.get(current_selected), Some(GroupedListItem::Item(_)))
    {
        return None;
    }

    coerce_selection(rows, first).or_else(|| coerce_selection(rows, last))
}

#[allow(dead_code)] // Shared with the launcher binary; the library target compiles this module without the owning call sites.
pub(crate) fn reanchor_uniform_selection(
    current_selected: usize,
    first_visible: usize,
    visible_items: usize,
    total_items: usize,
) -> Option<usize> {
    if total_items == 0 {
        return None;
    }

    let last_visible = (first_visible + visible_items.saturating_sub(1)).min(total_items - 1);
    if current_selected >= first_visible && current_selected <= last_visible {
        None
    } else {
        Some(first_visible.min(total_items - 1))
    }
}

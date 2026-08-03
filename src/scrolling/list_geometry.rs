//! Selection-neutral geometry for variable-height lists.

use gpui::{ListOffset, Pixels};

/// Resolved grouped-list heights for one explicit main-menu theme snapshot.
#[derive(Clone, Copy)]
pub(crate) struct GroupedListRowHeights {
    pub(crate) first_section_header: f32,
    pub(crate) section_header: f32,
    pub(crate) status: f32,
    pub(crate) item: f32,
}

impl GroupedListRowHeights {
    pub(crate) fn for_theme(theme: crate::designs::MainMenuThemeVariant) -> Self {
        Self {
            first_section_header: crate::list_item::effective_first_section_header_height_for_theme(
                theme,
            ),
            section_header: crate::list_item::effective_section_header_height_for_theme(theme),
            status: crate::list_item::effective_source_status_row_height_for_theme(theme),
            item: crate::list_item::effective_list_item_height_for_theme(theme),
        }
    }

    pub(crate) fn row_height(&self, row: &crate::list_item::GroupedListItem, ix: usize) -> f32 {
        match row {
            crate::list_item::GroupedListItem::SectionHeader(..) => {
                if ix == 0 {
                    self.first_section_header
                } else {
                    self.section_header
                }
            }
            crate::list_item::GroupedListItem::ReservedSectionSlot => self.first_section_header,
            crate::list_item::GroupedListItem::Status(..) => self.status,
            crate::list_item::GroupedListItem::Item(..) => self.item,
        }
    }
}

pub(crate) fn content_height(item_count: usize, mut row_height: impl FnMut(usize) -> f32) -> f32 {
    (0..item_count).map(|ix| row_height(ix).max(0.0)).sum()
}

pub(crate) fn pixel_top_for_item(
    item_count: usize,
    item_ix: usize,
    mut row_height: impl FnMut(usize) -> f32,
) -> f32 {
    (0..item_ix.min(item_count))
        .map(|ix| row_height(ix).max(0.0))
        .sum()
}

pub(crate) fn pixel_top_for_offset(
    item_count: usize,
    offset: ListOffset,
    mut row_height: impl FnMut(usize) -> f32,
) -> f32 {
    let item_ix = offset.item_ix.min(item_count);
    let row_top: f32 = (0..item_ix).map(|ix| row_height(ix).max(0.0)).sum();
    if item_ix == item_count {
        row_top
    } else {
        row_top
            + offset
                .offset_in_item
                .as_f32()
                .max(0.0)
                .min(row_height(item_ix).max(0.0))
    }
}

pub(crate) fn offset_for_pixel_top(
    item_count: usize,
    pixel_top: f32,
    mut row_height: impl FnMut(usize) -> f32,
) -> ListOffset {
    if item_count == 0 {
        return ListOffset {
            item_ix: 0,
            offset_in_item: gpui::px(0.0),
        };
    }

    let pixel_top = pixel_top.max(0.0);
    let mut accumulated = 0.0;
    for ix in 0..item_count {
        let height = row_height(ix).max(0.0);
        let bottom = accumulated + height;
        if pixel_top < bottom {
            return ListOffset {
                item_ix: ix,
                offset_in_item: gpui::px(pixel_top - accumulated),
            };
        }
        accumulated = bottom;
    }
    ListOffset {
        item_ix: item_count,
        offset_in_item: gpui::px(0.0),
    }
}

pub(crate) fn clamp_offset(
    item_count: usize,
    offset: ListOffset,
    mut row_height: impl FnMut(usize) -> f32,
) -> ListOffset {
    let item_ix = offset.item_ix.min(item_count);
    let offset_in_item = if item_ix == item_count {
        0.0
    } else {
        offset
            .offset_in_item
            .as_f32()
            .max(0.0)
            .min(row_height(item_ix).max(0.0))
    };
    ListOffset {
        item_ix,
        offset_in_item: gpui::px(offset_in_item),
    }
}

/// Resolve a captured viewport anchor against a replacement row set.
///
/// Stable identities win in capture order. If every captured row disappeared,
/// the old leading row index is clamped into the replacement list. Selection is
/// intentionally absent from this policy: viewport repair must never choose the
/// focused row.
pub(crate) fn restore_stable_viewport_offset(
    captured_keys: &[String],
    fallback_item_ix: usize,
    offset_in_item: Pixels,
    replacement_keys: &[String],
    mut row_height: impl FnMut(usize) -> f32,
) -> ListOffset {
    if replacement_keys.is_empty() {
        return ListOffset {
            item_ix: 0,
            offset_in_item: gpui::px(0.0),
        };
    }

    let item_ix = captured_keys
        .iter()
        .find_map(|captured| replacement_keys.iter().position(|key| key == captured))
        .unwrap_or_else(|| fallback_item_ix.min(replacement_keys.len() - 1));

    clamp_offset(
        replacement_keys.len(),
        ListOffset {
            item_ix,
            offset_in_item,
        },
        &mut row_height,
    )
}

pub(crate) fn first_rendered_item_at_or_after(
    item_count: usize,
    start_ix: usize,
    mut row_height: impl FnMut(usize) -> f32,
) -> Option<usize> {
    (start_ix.min(item_count)..item_count).find(|&ix| row_height(ix) > 0.0)
}

/// Returns a half-open range of rows intersecting the viewport.
pub(crate) fn visible_range(
    item_count: usize,
    scroll_top: f32,
    viewport_height: Pixels,
    mut row_height: impl FnMut(usize) -> f32,
) -> Option<(usize, usize)> {
    if item_count == 0 || viewport_height <= gpui::px(0.0) {
        return None;
    }
    let scroll_top = scroll_top.max(0.0);
    let visible_bottom = scroll_top + viewport_height.as_f32();
    let mut row_top = 0.0;
    let mut first = None;
    let mut last_exclusive = 0;
    for ix in 0..item_count {
        let row_bottom = row_top + row_height(ix).max(0.0);
        if row_bottom > scroll_top && row_top < visible_bottom {
            first.get_or_insert(ix);
            last_exclusive = ix + 1;
        }
        row_top = row_bottom;
        if row_top >= visible_bottom && first.is_some() {
            break;
        }
    }
    first.map(|first| (first, last_exclusive))
}

pub(crate) fn visible_range_for_offset(
    item_count: usize,
    offset: ListOffset,
    viewport_height: Pixels,
    mut row_height: impl FnMut(usize) -> f32,
) -> Option<(usize, usize)> {
    let scroll_top = pixel_top_for_offset(item_count, offset, &mut row_height);
    visible_range(item_count, scroll_top, viewport_height, row_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    enum FixtureRow {
        Header(f32),
        Status(f32),
        Item(f32),
    }

    const MIXED: &[FixtureRow] = &[
        FixtureRow::Header(0.0),
        FixtureRow::Header(24.0),
        FixtureRow::Status(32.0),
        FixtureRow::Item(40.0),
        FixtureRow::Item(48.0),
    ];

    fn height(ix: usize) -> f32 {
        match MIXED[ix] {
            FixtureRow::Header(height) | FixtureRow::Status(height) | FixtureRow::Item(height) => {
                height
            }
        }
    }

    fn assert_offset(actual: ListOffset, item_ix: usize, offset_in_item: f32) {
        assert_eq!(actual.item_ix, item_ix);
        assert_eq!(actual.offset_in_item, gpui::px(offset_in_item));
    }

    #[test]
    fn mixed_rows_include_hidden_zero_height_initial_header() {
        assert_eq!(content_height(MIXED.len(), height), 144.0);
        assert_eq!(pixel_top_for_item(MIXED.len(), 2, height), 24.0);
        assert_eq!(
            visible_range(MIXED.len(), 0.0, gpui::px(25.0), height),
            Some((1, 3))
        );
    }

    #[test]
    fn partial_first_item_round_trips_through_pixel_top() {
        let offset = ListOffset {
            item_ix: 2,
            offset_in_item: gpui::px(7.5),
        };
        let top = pixel_top_for_offset(MIXED.len(), offset, height);
        assert_eq!(top, 31.5);
        assert_offset(offset_for_pixel_top(MIXED.len(), top, height), 2, 7.5);
    }

    #[test]
    fn visible_range_is_half_open_at_first_and_last_rows() {
        assert_eq!(
            visible_range_for_offset(
                MIXED.len(),
                ListOffset {
                    item_ix: 2,
                    offset_in_item: gpui::px(8.0)
                },
                gpui::px(60.0),
                height,
            ),
            Some((2, 4))
        );
        assert_eq!(
            visible_range(MIXED.len(), 120.0, gpui::px(24.0), height),
            Some((4, 5))
        );
    }

    #[test]
    fn clamping_never_coerces_to_another_row() {
        assert_offset(
            clamp_offset(
                MIXED.len(),
                ListOffset {
                    item_ix: 3,
                    offset_in_item: gpui::px(99.0),
                },
                height,
            ),
            3,
            40.0,
        );
        assert_offset(
            clamp_offset(
                MIXED.len(),
                ListOffset {
                    item_ix: 99,
                    offset_in_item: gpui::px(12.0),
                },
                height,
            ),
            MIXED.len(),
            0.0,
        );
    }

    #[test]
    fn stable_anchor_survives_reorder_with_partial_offset() {
        let captured = vec!["b".to_string(), "c".to_string()];
        let replacement = vec!["c".to_string(), "a".to_string(), "b".to_string()];
        assert_offset(
            restore_stable_viewport_offset(&captured, 1, gpui::px(7.5), &replacement, |_| 40.0),
            2,
            7.5,
        );
    }

    #[test]
    fn deleted_anchor_uses_next_visible_identity_then_nearest_index_fallback() {
        let replacement = vec!["a".to_string(), "c".to_string(), "d".to_string()];
        assert_offset(
            restore_stable_viewport_offset(
                &["b".to_string(), "c".to_string()],
                1,
                gpui::px(4.0),
                &replacement,
                |_| 20.0,
            ),
            1,
            4.0,
        );
        assert_offset(
            restore_stable_viewport_offset(
                &["gone".to_string()],
                99,
                gpui::px(30.0),
                &replacement,
                |_| 20.0,
            ),
            2,
            20.0,
        );
    }

    #[test]
    fn zero_height_anchor_clamps_fraction_without_selecting_another_row() {
        let replacement = vec!["header".to_string(), "item".to_string()];
        assert_offset(
            restore_stable_viewport_offset(
                &["header".to_string()],
                0,
                gpui::px(8.0),
                &replacement,
                |ix| if ix == 0 { 0.0 } else { 40.0 },
            ),
            0,
            0.0,
        );
    }

    #[test]
    fn lazy_anchor_skips_zero_height_first_header() {
        assert_eq!(
            first_rendered_item_at_or_after(MIXED.len(), 0, height),
            Some(1)
        );
        assert_eq!(first_rendered_item_at_or_after(1, 1, |_| 40.0), None);
    }
}

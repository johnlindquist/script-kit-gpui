// App navigation methods - extracted from app_impl.rs
// This file is included via include!() macro in main.rs
// Contains: move_selection_up, move_selection_down, scroll_to_selected, etc.

#[inline]
fn page_down_target_index(
    grouped_items: &[GroupedListItem],
    selected_index: usize,
    page_size: usize,
    last_selectable: Option<usize>,
) -> usize {
    if page_size == 0 {
        return selected_index;
    }

    let Some(last_selectable) = last_selectable else {
        return selected_index;
    };

    let selected_index = selected_index.min(last_selectable);

    if selected_index >= last_selectable {
        return selected_index;
    }

    let mut remaining = page_size;
    let mut target = selected_index;
    for i in (selected_index + 1)..=last_selectable {
        if matches!(grouped_items.get(i), Some(GroupedListItem::Item(_))) {
            target = i;
            remaining -= 1;
            if remaining == 0 {
                break;
            }
        }
    }

    target
}

#[inline]
fn validated_selection_index(grouped_items: &[GroupedListItem], selected_index: usize) -> usize {
    list_item::coerce_selection(grouped_items, selected_index).unwrap_or(0)
}

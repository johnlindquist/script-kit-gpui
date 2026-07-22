#[cfg(test)]
mod app_navigation_tests {
    use super::{page_down_target_index, validated_selection_index};
    use crate::list_item::GroupedListItem;

    #[test]
    fn test_move_selection_page_down_clamps_to_last_item() {
        let rows = vec![
            GroupedListItem::SectionHeader("Suggested".to_string(), None),
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
            GroupedListItem::SectionHeader("Main".to_string(), None),
            GroupedListItem::Item(2),
        ];

        assert_eq!(page_down_target_index(&rows, 1, 10, Some(4)), 4);
        assert_eq!(page_down_target_index(&rows, 4, 10, Some(4)), 4);
        assert_eq!(page_down_target_index(&rows, usize::MAX, 10, Some(4)), 4);
    }

    #[test]
    fn test_move_selection_page_down_noops_when_page_size_is_zero() {
        let rows = vec![
            GroupedListItem::SectionHeader("Suggested".to_string(), None),
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
        ];

        assert_eq!(page_down_target_index(&rows, 1, 0, Some(2)), 1);
    }

    #[test]
    fn test_validate_selection_bounds_recovers_from_out_of_range_index() {
        let rows = vec![
            GroupedListItem::SectionHeader("Suggested".to_string(), None),
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
            GroupedListItem::SectionHeader("Main".to_string(), None),
            GroupedListItem::Item(2),
        ];
        let headers_only = vec![
            GroupedListItem::SectionHeader("A".to_string(), None),
            GroupedListItem::SectionHeader("B".to_string(), None),
        ];

        assert_eq!(validated_selection_index(&rows, 999), 4);
        assert_eq!(validated_selection_index(&rows, 0), 1);
        assert_eq!(validated_selection_index(&headers_only, 4), 0);
    }
}

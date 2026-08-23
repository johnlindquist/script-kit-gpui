#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emoji_database_has_296_entries() {
        assert_eq!(EMOJIS.len(), 296);
    }

    #[test]
    fn test_emoji_category_display_name_returns_human_readable_labels() {
        assert_eq!(SmileysEmotion.display_name(), "Smileys & Emotion");
        assert_eq!(PeopleBody.display_name(), "People & Body");
        assert_eq!(AnimalsNature.display_name(), "Animals & Nature");
        assert_eq!(FoodDrink.display_name(), "Food & Drink");
        assert_eq!(TravelPlaces.display_name(), "Travel & Places");
        assert_eq!(Activities.display_name(), "Activities");
        assert_eq!(Objects.display_name(), "Objects");
        assert_eq!(Symbols.display_name(), "Symbols");
        assert_eq!(Flags.display_name(), "Flags");
    }

    #[test]
    fn test_all_categories_has_expected_display_order() {
        let categories: Vec<EmojiCategory> = all_categories().collect();
        assert_eq!(
            categories,
            vec![
                SmileysEmotion,
                PeopleBody,
                AnimalsNature,
                FoodDrink,
                TravelPlaces,
                Activities,
                Objects,
                Symbols,
                Flags
            ]
        );
    }

    #[test]
    fn test_emojis_by_category_returns_only_requested_category() {
        let travel_emojis = emojis_by_category(TravelPlaces);
        assert!(!travel_emojis.is_empty());
        assert!(travel_emojis
            .iter()
            .all(|emoji| emoji.category == TravelPlaces));
    }

    #[test]
    fn test_grouped_emojis_returns_all_categories_in_display_order() {
        let grouped = grouped_emojis();
        assert_eq!(grouped.len(), all_categories().count());

        for ((category, emojis), expected_category) in grouped.iter().zip(all_categories()) {
            assert_eq!(*category, expected_category);
            assert!(emojis
                .iter()
                .all(|emoji| emoji.category == expected_category));
        }
    }

    #[test]
    fn test_grouped_emojis_covers_all_entries() {
        let grouped = grouped_emojis();
        let total_grouped_emojis: usize = grouped.iter().map(|(_, emojis)| emojis.len()).sum();
        assert_eq!(total_grouped_emojis, EMOJIS.len());
    }

    #[test]
    fn test_emoji_database_meets_category_targets() {
        assert!(emojis_by_category(SmileysEmotion).len() >= 50);
        assert!(emojis_by_category(PeopleBody).len() >= 30);
        assert!(emojis_by_category(AnimalsNature).len() >= 20);
        assert!(emojis_by_category(FoodDrink).len() >= 15);
        assert!(emojis_by_category(TravelPlaces).len() >= 15);
        assert!(emojis_by_category(Activities).len() >= 10);
        assert!(emojis_by_category(Objects).len() >= 15);
        assert!(emojis_by_category(Symbols).len() >= 15);
        assert!(emojis_by_category(Flags).len() >= 10);
    }

    #[test]
    fn test_search_emojis_matches_name_when_query_has_different_case() {
        let matches = search_emojis("GRINNING");
        assert!(matches.iter().any(|emoji| emoji.emoji == "😀"));
    }

    #[test]
    fn test_search_emojis_matches_keyword_when_query_is_substring() {
        let matches = search_emojis("appro");
        assert!(matches.iter().any(|emoji| emoji.emoji == "👍"));
    }

    #[test]
    fn test_search_emojis_returns_all_when_query_is_empty() {
        let matches = search_emojis("   ");
        assert_eq!(matches.len(), EMOJIS.len());
    }

    #[test]
    fn test_filtered_grid_row_count_matches_current_dataset() {
        // Unfiltered: 9 category headers + cell rows for all 296 emojis
        let total = filtered_grid_row_count("", None);
        assert!(total > 0, "unfiltered grid should have rows");

        // "heart" filter should return a smaller count
        let heart = filtered_grid_row_count("heart", None);
        assert!(
            heart < total,
            "heart filter should have fewer rows than unfiltered"
        );
        assert!(heart > 0, "heart filter should have some rows");

        // "pizza" filter should be very small
        let pizza = filtered_grid_row_count("pizza", None);
        assert!(pizza > 0 && pizza <= 4, "pizza filter should have 1-4 rows");
    }

    #[test]
    fn emoji_grid_layout_moves_across_ragged_rows() {
        let layout = EmojiGridLayout {
            rows: vec![
                EmojiCellRow {
                    visible_row_index: 1,
                    start_index: 0,
                    count: 4,
                },
                EmojiCellRow {
                    visible_row_index: 2,
                    start_index: 4,
                    count: 1,
                },
                EmojiCellRow {
                    visible_row_index: 4,
                    start_index: 5,
                    count: 4,
                },
            ],
            item_to_row: vec![0, 0, 0, 0, 1, 2, 2, 2, 2],
        };

        // Down from full row into short row (column clamping: col 3 → col 0)
        assert_eq!(layout.move_index(3, EmojiNavDirection::Down), 4);
        // Down from short row into next full row
        assert_eq!(layout.move_index(4, EmojiNavDirection::Down), 5);
        // Up from full row into short row with column clamping (col 3 → col 0)
        assert_eq!(layout.move_index(8, EmojiNavDirection::Up), 4);
        // scroll_row_for_index returns correct visible_row_index
        assert_eq!(layout.scroll_row_for_index(5), 4);
        // Left wrapping to previous row's last item
        assert_eq!(
            layout.move_index(5, EmojiNavDirection::Left),
            4,
            "Left from first cell of row 2 should wrap to last cell of row 1"
        );
        // Right wrapping to next row's first item
        assert_eq!(
            layout.move_index(4, EmojiNavDirection::Right),
            5,
            "Right from last cell of short row should wrap to first cell of next row"
        );
    }

    #[test]
    fn emoji_grid_layout_build_produces_correct_rows() {
        // Build layout from real emoji data and verify structure
        let ordered = filtered_ordered_emojis("", None);
        let layout = build_emoji_grid_layout(&ordered, GRID_COLS, |e| e.category);

        // Every item should map to a valid row
        for (i, &row_ix) in layout.item_to_row.iter().enumerate() {
            assert!(
                row_ix < layout.rows.len(),
                "item {i} maps to out-of-bounds row {row_ix}"
            );
            let row = &layout.rows[row_ix];
            assert!(
                i >= row.start_index && i < row.start_index + row.count,
                "item {i} not within its mapped row (start={}, count={})",
                row.start_index,
                row.count
            );
        }
    }

    /// Locks the balanced emoji-grid rhythm: larger glyph-bearing tiles,
    /// enough columns to use the window width, and equal horizontal and
    /// vertical center-to-center spacing (`tile + gap == row height`).
    #[test]
    fn test_emoji_picker_grid_layout_constants_match_density_targets() {
        assert_eq!(GRID_COLS, 12);
        assert_eq!(GRID_TILE_SIZE, 48.0);
        assert_eq!(GRID_TILE_GAP, 8.0);
        assert_eq!(GRID_ROW_HEIGHT, 56.0);
        assert_eq!(GRID_TILE_SIZE + GRID_TILE_GAP, GRID_ROW_HEIGHT);
        assert_eq!(GRID_GLYPH_SCALE, 0.75);
        assert_eq!(GRID_TILE_SIZE * GRID_GLYPH_SCALE, 36.0);
    }

    #[test]
    fn test_grid_layout_scroll_rows_are_monotonic() {
        let ordered = filtered_ordered_emojis("", None);
        assert!(!ordered.is_empty(), "emoji dataset should not be empty");

        let layout = build_emoji_grid_layout(&ordered, GRID_COLS, |emoji| emoji.category);
        let mut last_row = layout.scroll_row_for_index(0);

        for ix in 1..ordered.len() {
            let row = layout.scroll_row_for_index(ix);
            assert!(
                row >= last_row,
                "scroll row must be monotonic: ix={} row={} previous_row={}",
                ix,
                row,
                last_row
            );
            last_row = row;
        }
    }

    #[test]
    fn test_grid_layout_left_and_right_clamp_at_edges() {
        let ordered = filtered_ordered_emojis("", None);
        assert!(!ordered.is_empty(), "emoji dataset should not be empty");

        let layout = build_emoji_grid_layout(&ordered, GRID_COLS, |emoji| emoji.category);
        assert_eq!(layout.move_index(0, EmojiNavDirection::Left), 0);

        let last = ordered.len() - 1;
        assert_eq!(layout.move_index(last, EmojiNavDirection::Right), last);
    }

    #[test]
    fn display_grid_layout_maps_frequent_head_block_to_single_section() {
        // Choose two emoji values from different categories so the head
        // block would be split by the category-aware grouping if it
        // weren't handled as its own section. The frequent block must
        // map to one contiguous region above the first category header.
        let frequent = vec!["❤️".to_string(), "🔥".to_string()];
        let display = display_ordered_emojis("", None, &frequent);
        assert_eq!(display.frequent_count, 2);

        let layout = build_display_grid_layout(&display, GRID_COLS);

        // First two items live in the same head row: a single cell row
        // whose visible_row_index == 1 (header row is 0).
        let row_0 = layout.item_to_row[0];
        let row_1 = layout.item_to_row[1];
        assert_eq!(row_0, row_1, "frequent head must be one contiguous row");
        assert_eq!(layout.rows[row_0].visible_row_index, 1);
        assert_eq!(layout.rows[row_0].start_index, 0);
        assert_eq!(layout.rows[row_0].count, 2);

        // Item 2 is the first category cell — it must sit below a
        // category header row (visible_row_index jumps by at least 2:
        // one for the end of the head block and one for the category
        // header).
        let row_2 = layout.item_to_row[2];
        assert!(
            layout.rows[row_2].visible_row_index >= 3,
            "first category cell must sit below the Frequently Used \
             head block AND its own category header row"
        );

        // Navigating Down from the head block must land on a category
        // cell (not on a header row).
        let down = layout.move_index(0, EmojiNavDirection::Down);
        assert!(
            down >= display.frequent_count,
            "Down from the head block must enter the category region, \
             landed at {down} (frequent_count={})",
            display.frequent_count
        );
        // And the reverse direction round-trips cleanly.
        let up = layout.move_index(down, EmojiNavDirection::Up);
        assert!(
            up < display.frequent_count,
            "Up from first category cell must re-enter the Frequently \
             Used head block, landed at {up}"
        );
    }

    #[test]
    fn display_grid_layout_matches_build_emoji_grid_when_no_frequent() {
        // With an empty frequent list the new builder must behave
        // identically to the legacy builder. This guards the "no regression
        // when the feature is dormant" invariant.
        let display = display_ordered_emojis("", None, &[]);
        assert_eq!(display.frequent_count, 0);

        let base = filtered_ordered_emojis("", None);
        let legacy = build_emoji_grid_layout(&base, GRID_COLS, |e| e.category);
        let new = build_display_grid_layout(&display, GRID_COLS);

        assert_eq!(legacy.rows, new.rows);
        assert_eq!(legacy.item_to_row, new.item_to_row);
    }

    #[test]
    fn compute_display_scroll_row_matches_layout_visible_row_index() {
        // With a frequent head block the single-step scroll helper must
        // agree with the layout's visible_row_index for every item.
        let frequent = vec!["❤️".to_string(), "🔥".to_string(), "🎉".to_string()];
        let display = display_ordered_emojis("", None, &frequent);
        let layout = build_display_grid_layout(&display, GRID_COLS);

        for ix in 0..display.emojis.len() {
            let expected = layout.scroll_row_for_index(ix);
            let actual = compute_display_scroll_row(ix, &display);
            assert_eq!(
                actual, expected,
                "scroll row mismatch at ix={} (expected {expected}, got {actual})",
                ix
            );
        }
    }

    #[test]
    fn test_grid_layout_down_then_up_round_trips_when_destination_exists() {
        let ordered = filtered_ordered_emojis("", None);
        assert!(
            ordered.len() > GRID_COLS,
            "need at least two rows of emoji data for navigation coverage"
        );

        let layout = build_emoji_grid_layout(&ordered, GRID_COLS, |emoji| emoji.category);

        let mut found_round_trip = false;
        for ix in 0..(ordered.len() - GRID_COLS) {
            let down = layout.move_index(ix, EmojiNavDirection::Down);
            if down != ix {
                found_round_trip = true;
                assert_eq!(
                    layout.move_index(down, EmojiNavDirection::Up),
                    ix,
                    "down/up should round-trip when a destination cell exists: start={} down={}",
                    ix,
                    down
                );
                break;
            }
        }

        assert!(
            found_round_trip,
            "expected at least one index with a valid downward move"
        );
    }
}

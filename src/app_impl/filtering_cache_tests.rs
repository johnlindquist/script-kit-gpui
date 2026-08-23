#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_passive_search_needle_preserves_under_boundary_query() {
        let query = "a".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS - 1);

        assert_eq!(root_passive_search_needle(&query), query);
    }

    #[test]
    fn root_passive_search_needle_preserves_exact_boundary_query() {
        let query = "a".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS);

        assert_eq!(root_passive_search_needle(&query), query);
    }

    #[test]
    fn root_passive_search_needle_clamps_over_boundary_ascii_query() {
        let query = "a".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS + 1);
        let needle = root_passive_search_needle(&query);

        assert_eq!(needle, "a".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS));
        assert_eq!(needle.chars().count(), ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS);
    }

    #[test]
    fn root_passive_search_needle_clamps_at_multibyte_scalar_boundary() {
        let query = format!(
            "{}🦀trailing",
            "a".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS - 1)
        );
        let needle = root_passive_search_needle(&query);

        assert!(needle.ends_with('🦀'));
        assert_eq!(needle.chars().count(), ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS);
        assert_eq!(needle, &query[..needle.len()]);
    }

    #[test]
    fn root_passive_search_needle_leaves_original_full_string_unchanged() {
        let query = format!(
            "{}full-tail",
            "界".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS)
        );
        let original = query.clone();
        let needle = root_passive_search_needle(&query).to_string();

        assert_eq!(query, original);
        assert_eq!(needle, "界".repeat(ROOT_PASSIVE_SEARCH_NEEDLE_MAX_CHARS));
        assert!(query.ends_with("full-tail"));
    }

    fn calculator_result() -> crate::calculator::CalculatorInlineResult {
        crate::calculator::CalculatorInlineResult {
            raw_input: "12 / 3".to_string(),
            normalized_expr: "12 / 3".to_string(),
            operation_name: "Divide".to_string(),
            value: 4.0,
            formatted: "4".to_string(),
            words: "Four".to_string(),
        }
    }

    #[test]
    fn test_prepend_inline_calculator_group_prepends_header_and_item() {
        let grouped_items = vec![
            GroupedListItem::SectionHeader("Suggested".to_string(), None),
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
        ];
        let flat_results = Vec::new();

        let (grouped, flat) = prepend_inline_calculator_group(
            grouped_items,
            flat_results,
            Some(&calculator_result()),
        );

        assert!(matches!(
            grouped.first(),
            Some(GroupedListItem::SectionHeader(label, None))
            if label == INLINE_CALCULATOR_SECTION_LABEL
        ));
        assert!(matches!(
            grouped.get(1),
            Some(GroupedListItem::Item(INLINE_CALCULATOR_RESULT_INDEX))
        ));
        assert!(matches!(
            grouped.get(2),
            Some(GroupedListItem::SectionHeader(_, _))
        ));
        assert!(matches!(grouped.get(3), Some(GroupedListItem::Item(0))));
        assert!(matches!(grouped.get(4), Some(GroupedListItem::Item(1))));
        assert!(flat.is_empty());
    }

    #[test]
    fn test_prepend_inline_calculator_group_is_noop_without_calculator() {
        let grouped_items = vec![GroupedListItem::Item(0)];
        let flat_results = Vec::new();

        let (grouped, flat) = prepend_inline_calculator_group(grouped_items, flat_results, None);

        assert_eq!(grouped.len(), 1);
        assert!(matches!(grouped.first(), Some(GroupedListItem::Item(0))));
        assert!(flat.is_empty());
    }

    #[test]
    fn test_apply_match_emphasis_handles_indented_match_offsets() {
        let mut line = syntax::HighlightedLine {
            spans: vec![syntax::HighlightedSpan::new(
                "    const superUniqueToken = value;",
                0xcccccc,
            )],
        };

        let leading_ws_chars = 4;
        let snippet_match_start = 6;
        let snippet_match_end = 22;
        ScriptListApp::apply_match_emphasis_to_line(
            &mut line,
            leading_ws_chars + snippet_match_start,
            leading_ws_chars + snippet_match_end,
        );

        let emphasized: String = line
            .spans
            .iter()
            .filter(|span| span.is_match_emphasis)
            .map(|span| span.text.as_str())
            .collect();
        assert_eq!(emphasized, "superUniqueToken");
    }

    #[test]
    fn preview_match_signature_changes_when_byte_range_changes() {
        let alpha = scripts::ScriptContentMatch {
            line_number: 4,
            line_text: "const alpha = beta;".to_string(),
            line_match_indices: vec![6, 7, 8, 9, 10],
            byte_range: 20..25,
        };
        let beta = scripts::ScriptContentMatch {
            line_number: 4,
            line_text: "const alpha = beta;".to_string(),
            line_match_indices: vec![14, 15, 16, 17],
            byte_range: 28..32,
        };
        assert_ne!(
            scripts::preview_match_signature(Some(&alpha)),
            scripts::preview_match_signature(Some(&beta))
        );
    }

    #[test]
    fn preview_match_signature_is_none_without_content_match() {
        assert_eq!(scripts::preview_match_signature(None), None);
    }

    #[test]
    fn preview_cache_is_valid_for_identical_match_signature() {
        let alpha = scripts::ScriptContentMatch {
            line_number: 1,
            line_text: "const alpha = beta;".to_string(),
            line_match_indices: vec![6, 7, 8, 9, 10],
            byte_range: 6..11,
        };
        assert!(scripts::preview_cache_is_valid(
            Some("/tmp/demo.ts"),
            scripts::preview_match_signature(Some(&alpha)),
            false, // cached_lines_empty
            "/tmp/demo.ts",
            Some(&alpha),
        ));
    }

    #[test]
    fn preview_cache_is_invalid_when_same_line_match_moves_to_new_span() {
        let alpha = scripts::ScriptContentMatch {
            line_number: 1,
            line_text: "const alpha = beta;".to_string(),
            line_match_indices: vec![6, 7, 8, 9, 10],
            byte_range: 6..11,
        };
        let beta = scripts::ScriptContentMatch {
            line_number: 1,
            line_text: "const alpha = beta;".to_string(),
            line_match_indices: vec![14, 15, 16, 17],
            byte_range: 14..18,
        };
        assert!(!scripts::preview_cache_is_valid(
            Some("/tmp/demo.ts"),
            scripts::preview_match_signature(Some(&alpha)),
            false, // cached_lines_empty
            "/tmp/demo.ts",
            Some(&beta),
        ));
    }

    #[test]
    fn preview_cache_is_invalid_when_cached_lines_are_empty() {
        let alpha = scripts::ScriptContentMatch {
            line_number: 1,
            line_text: "const alpha = beta;".to_string(),
            line_match_indices: vec![6, 7, 8, 9, 10],
            byte_range: 6..11,
        };
        assert!(!scripts::preview_cache_is_valid(
            Some("/tmp/demo.ts"),
            scripts::preview_match_signature(Some(&alpha)),
            true, // cached_lines_empty
            "/tmp/demo.ts",
            Some(&alpha),
        ));
    }

    #[test]
    fn empty_context_subsearch_prefix_routes_to_guarded_rich_rows() {
        for (input, expected_source) in [
            (
                "@file:",
                crate::spine::catalog_subsearch::ContextSubsearchSource::File,
            ),
            (
                "@clipboard:",
                crate::spine::catalog_subsearch::ContextSubsearchSource::Clipboard,
            ),
            (
                "@history:",
                crate::spine::catalog_subsearch::ContextSubsearchSource::History,
            ),
            (
                "@browser-history:",
                crate::spine::catalog_subsearch::ContextSubsearchSource::BrowserHistory,
            ),
        ] {
            let parse = crate::spine::parse_spine(input);
            let projection = crate::spine::project_cursor(&parse, input.len());

            assert_eq!(
                active_rich_spine_subsearch(&projection),
                Some((expected_source, String::new())),
                "{input} must route to rich rows so unarmed recents render with the choose hint"
            );
        }
    }

    #[test]
    fn rich_browser_history_is_recognized_before_cold_snapshot_refresh() {
        assert!(active_rich_browser_history_subsearch("@browser-history:"));
        assert!(active_rich_browser_history_subsearch(
            "@browser-history:secret"
        ));
        assert!(active_rich_browser_history_subsearch("@browser-history"));
        assert!(!active_rich_browser_history_subsearch("@history:"));
        assert!(!active_rich_browser_history_subsearch("ordinary query"));
    }

    /// Colon-less root fragments must auto-enter the same guarded rich rows —
    /// typing `@files` IS file search; no "press Enter to refine" picker step
    /// and no informational list items (user rule).
    #[test]
    fn root_context_fragment_routes_to_guarded_rich_rows() {
        for (input, expected_source, expected_query) in [
            (
                "@files",
                crate::spine::catalog_subsearch::ContextSubsearchSource::File,
                "",
            ),
            (
                "@filesreadme",
                crate::spine::catalog_subsearch::ContextSubsearchSource::File,
                "readme",
            ),
            (
                "@clipboard",
                crate::spine::catalog_subsearch::ContextSubsearchSource::Clipboard,
                "",
            ),
            (
                "@history",
                crate::spine::catalog_subsearch::ContextSubsearchSource::History,
                "",
            ),
            (
                "@browser-history",
                crate::spine::catalog_subsearch::ContextSubsearchSource::BrowserHistory,
                "",
            ),
        ] {
            let parse = crate::spine::parse_spine(input);
            let projection = crate::spine::project_cursor(&parse, input.len());

            assert_eq!(
                active_rich_spine_subsearch(&projection),
                Some((expected_source, expected_query.to_string())),
                "{input} must auto-route to rich search rows"
            );
        }
    }

    #[test]
    fn empty_subsearch_choose_hint_rides_first_header_and_adds_no_row() {
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Recent Files".to_string(), Some("file".to_string())),
            GroupedListItem::Item(0),
        ];

        append_choose_hint_to_first_section_header(&mut grouped);

        assert_eq!(
            grouped.len(),
            2,
            "the choose hint must not add list rows; it rides the existing header"
        );
        let Some(GroupedListItem::SectionHeader(label, _)) = grouped.first() else {
            panic!("first grouped entry must remain the section header");
        };
        assert_eq!(label, "Recent Files \u{b7} \u{2193} to choose");
        assert!(matches!(grouped.get(1), Some(GroupedListItem::Item(0))));
    }

    #[test]
    fn choose_hint_skipped_when_no_selectable_rows() {
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Files".to_string(), Some("file".to_string())),
            GroupedListItem::SectionHeader("No recent files".to_string(), None),
        ];

        append_choose_hint_to_first_section_header(&mut grouped);

        let Some(GroupedListItem::SectionHeader(label, _)) = grouped.first() else {
            panic!("first grouped entry must remain the section header");
        };
        assert_eq!(
            label, "Files",
            "an empty list must not advertise \u{2193} to choose"
        );
    }

    #[test]
    fn typed_context_subsearch_arms_rich_rows() {
        for (input, expected_source, expected_query) in [
            (
                "@file:readme",
                crate::spine::catalog_subsearch::ContextSubsearchSource::File,
                "readme",
            ),
            (
                "@clipboard:snippet",
                crate::spine::catalog_subsearch::ContextSubsearchSource::Clipboard,
                "snippet",
            ),
            (
                "@history:agent",
                crate::spine::catalog_subsearch::ContextSubsearchSource::History,
                "agent",
            ),
        ] {
            let parse = crate::spine::parse_spine(input);
            let projection = crate::spine::project_cursor(&parse, input.len());

            assert_eq!(
                active_rich_spine_subsearch(&projection),
                Some((expected_source, expected_query.to_string())),
                "{input} should still route typed subqueries to native rich rows"
            );
        }
    }
}

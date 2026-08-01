//! Unit tests for UnifiedListItem component types and layout helpers.

use super::{Density, ListItemLayout, SECTION_HEADER_HEIGHT, TextContent, TrailingContent};

#[test]
fn highlighted_text_preserves_source_text_and_ranges() {
    let content = TextContent::highlighted("Hello World", vec![0..5, 6..11]);

    assert_eq!(content.as_str(), Some("Hello World"));

    match content {
        TextContent::Highlighted { ranges, .. } => assert_eq!(ranges, vec![0..5, 6..11]),
        _ => panic!("expected highlighted text content"),
    }
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "char boundary")]
fn highlighted_text_rejects_invalid_utf8_boundaries() {
    // "a😀b" => valid boundaries are 0,1,5,6. Start=2 is invalid.
    let _ = TextContent::highlighted("a😀b", std::iter::once(2..5).collect());
}

#[test]
fn layout_density_scales_spacing_but_keeps_canonical_height() {
    let comfortable = ListItemLayout::from_density(Density::Comfortable);
    let compact = ListItemLayout::from_density(Density::Compact);

    assert_eq!(comfortable.height, crate::list_item::LIST_ITEM_HEIGHT);
    assert_eq!(compact.height, crate::list_item::LIST_ITEM_HEIGHT);
    assert!(comfortable.padding_x > compact.padding_x);
    assert!(comfortable.padding_y > compact.padding_y);
    assert!(comfortable.leading_size > compact.leading_size);
    assert!(comfortable.gap > compact.gap);
}

#[test]
fn compact_density_layout_matches_expected_tokens() {
    let compact = ListItemLayout::from_density(Density::Compact);

    assert_eq!(compact.padding_x, 8.0);
    assert_eq!(compact.padding_y, 4.0);
    assert_eq!(compact.gap, 6.0);
    assert_eq!(compact.leading_size, 16.0);
    assert_eq!(compact.radius, 4.0);
}

#[test]
fn section_header_height_matches_shared_constant() {
    assert_eq!(
        SECTION_HEADER_HEIGHT,
        crate::list_item::SECTION_HEADER_HEIGHT
    );
}

#[test]
fn test_highlighted_text_precomputes_fragments_when_constructed() {
    let content = TextContent::highlighted("Hello World", vec![0..5, 6..11]);

    let fragments = content
        .highlight_fragments()
        .expect("highlighted content should have fragments");
    assert_eq!(fragments.len(), 3);
    assert_eq!(fragments[0].text, "Hello");
    assert!(fragments[0].is_highlighted);
    assert_eq!(fragments[1].text, " ");
    assert!(!fragments[1].is_highlighted);
    assert_eq!(fragments[2].text, "World");
    assert!(fragments[2].is_highlighted);
}

#[test]
fn test_plain_text_has_no_highlight_fragments() {
    let content = TextContent::plain("No highlights");
    assert!(content.highlight_fragments().is_none());
}

#[test]
fn trailing_shortcut_caches_canonical_shared_tokens() {
    let content = TrailingContent::shortcut("Cmd++");
    match content {
        TrailingContent::Shortcut { raw, tokens } => {
            assert_eq!(raw, "Cmd++");
            assert_eq!(tokens.as_ref(), &["⌘".to_string(), "+".to_string()]);
        }
        _ => panic!("expected shortcut trailing content"),
    }
}

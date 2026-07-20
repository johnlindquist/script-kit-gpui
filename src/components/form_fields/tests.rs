//! Unit tests for form field text indexing helpers.
//!
//! These tests verify the UTF-8 char/byte conversion functions used by form fields.
//! Separated from form_fields.rs due to GPUI macro recursion limit issues.

use super::{form_field_type_allows_candidate_value, FormFieldColors, FormFieldMetrics};
use crate::designs::DesignColors;

/// Count the number of Unicode scalar values (chars) in a string.
fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Convert a character index (0..=char_len) into a byte index (0..=s.len()).
/// If char_idx is past the end, returns s.len().
fn byte_idx_from_char_idx(s: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }
    s.char_indices()
        .nth(char_idx)
        .map(|(byte_idx, _)| byte_idx)
        .unwrap_or_else(|| s.len())
}

/// Remove a char range [start_char, end_char) from a String (char indices).
fn drain_char_range(s: &mut String, start_char: usize, end_char: usize) {
    let start_b = byte_idx_from_char_idx(s, start_char);
    let end_b = byte_idx_from_char_idx(s, end_char);
    if start_b < end_b && start_b <= s.len() && end_b <= s.len() {
        s.drain(start_b..end_b);
    }
}

/// Slice a &str by char indices [start_char, end_char).
fn slice_by_char_range(s: &str, start_char: usize, end_char: usize) -> &str {
    let start_b = byte_idx_from_char_idx(s, start_char);
    let end_b = byte_idx_from_char_idx(s, end_char);
    &s[start_b..end_b]
}

// --- Text indexing helper tests ---

#[test]
fn test_byte_idx_from_char_idx_ascii() {
    let s = "hello";
    assert_eq!(byte_idx_from_char_idx(s, 0), 0);
    assert_eq!(byte_idx_from_char_idx(s, 1), 1);
    assert_eq!(byte_idx_from_char_idx(s, 5), 5);
    // Past end
    assert_eq!(byte_idx_from_char_idx(s, 10), 5);
}

#[test]
fn test_byte_idx_from_char_idx_emoji() {
    let s = "a😀b"; // a=1 byte, 😀=4 bytes, b=1 byte
    assert_eq!(byte_idx_from_char_idx(s, 0), 0); // before 'a'
    assert_eq!(byte_idx_from_char_idx(s, 1), 1); // before '😀'
    assert_eq!(byte_idx_from_char_idx(s, 2), 5); // before 'b' (1+4)
    assert_eq!(byte_idx_from_char_idx(s, 3), 6); // end
}

#[test]
fn test_byte_idx_from_char_idx_bullet() {
    let s = "•••"; // 3 bullets, each 3 bytes = 9 bytes total
    assert_eq!(byte_idx_from_char_idx(s, 0), 0);
    assert_eq!(byte_idx_from_char_idx(s, 1), 3);
    assert_eq!(byte_idx_from_char_idx(s, 2), 6);
    assert_eq!(byte_idx_from_char_idx(s, 3), 9);
}

#[test]
fn test_slice_by_char_range_ascii() {
    let s = "hello";
    assert_eq!(slice_by_char_range(s, 0, 2), "he");
    assert_eq!(slice_by_char_range(s, 2, 5), "llo");
    assert_eq!(slice_by_char_range(s, 0, 5), "hello");
}

#[test]
fn test_slice_by_char_range_emoji() {
    let s = "a😀b";
    assert_eq!(slice_by_char_range(s, 0, 1), "a");
    assert_eq!(slice_by_char_range(s, 1, 2), "😀");
    assert_eq!(slice_by_char_range(s, 2, 3), "b");
    assert_eq!(slice_by_char_range(s, 0, 3), "a😀b");
}

#[test]
fn test_slice_by_char_range_bullet() {
    let s = "•••";
    assert_eq!(slice_by_char_range(s, 0, 1), "•");
    assert_eq!(slice_by_char_range(s, 1, 2), "•");
    assert_eq!(slice_by_char_range(s, 0, 2), "••");
}

#[test]
fn test_drain_char_range_ascii() {
    let mut s = "hello".to_string();
    drain_char_range(&mut s, 1, 3);
    assert_eq!(s, "hlo");
}

#[test]
fn test_drain_char_range_emoji() {
    let mut s = "a😀b".to_string();
    drain_char_range(&mut s, 1, 2); // remove emoji
    assert_eq!(s, "ab");
}

#[test]
fn test_drain_char_range_bullet() {
    let mut s = "•••".to_string();
    drain_char_range(&mut s, 1, 2); // remove middle bullet
    assert_eq!(s, "••");
}

// --- Password bullet rendering tests ---

/// Test that password bullet string can be safely sliced by char index.
/// This test verifies the FIX for the bug where render() slices bullet
/// strings using cursor_position directly (which is a char index).
#[test]
fn test_password_bullet_slicing_safe() {
    let password = "abc"; // 3 chars
    let bullets = "•".repeat(char_len(password)); // "•••" = 9 bytes
    let cursor_pos: usize = 2; // char index

    // This is the CORRECT way to slice (using char indices):
    let before = slice_by_char_range(&bullets, 0, cursor_pos);
    let after = slice_by_char_range(&bullets, cursor_pos, char_len(&bullets));

    assert_eq!(before, "••");
    assert_eq!(after, "•");
}

#[test]
fn test_form_fields_use_theme_token_font_sizes() {
    let colors_source = std::fs::read_to_string("src/components/form_fields/colors.rs")
        .expect("failed to read src/components/form_fields/colors.rs");
    let text_field_render_source =
        std::fs::read_to_string("src/components/form_fields/text_field/render.rs")
            .expect("failed to read src/components/form_fields/text_field/render.rs");
    let text_area_render_source =
        std::fs::read_to_string("src/components/form_fields/text_area/render.rs")
            .expect("failed to read src/components/form_fields/text_area/render.rs");
    let checkbox_source = std::fs::read_to_string("src/components/form_fields/checkbox.rs")
        .expect("failed to read src/components/form_fields/checkbox.rs");
    let all_sources = format!(
        "{colors_source}\n{text_field_render_source}\n{text_area_render_source}\n{checkbox_source}"
    );

    assert!(
        colors_source.contains("input_font_size"),
        "form field colors should define an input font-size token"
    );
    assert!(
        colors_source.contains("label_font_size"),
        "form field colors should define a label font-size token"
    );
    assert!(
        all_sources.contains(".text_size(px(colors.input_font_size))"),
        "text inputs should use the shared input font-size token"
    );
    assert!(
        all_sources.contains(".text_size(px(colors.label_font_size))"),
        "labels should use the shared label font-size token"
    );
}

#[test]
fn test_form_fields_use_shared_metrics_for_layout_tokens() {
    let colors_source = std::fs::read_to_string("src/components/form_fields/colors.rs")
        .expect("failed to read src/components/form_fields/colors.rs");
    let text_field_render_source =
        std::fs::read_to_string("src/components/form_fields/text_field/render.rs")
            .expect("failed to read src/components/form_fields/text_field/render.rs");
    let text_area_render_source =
        std::fs::read_to_string("src/components/form_fields/text_area/render.rs")
            .expect("failed to read src/components/form_fields/text_area/render.rs");
    let checkbox_source = std::fs::read_to_string("src/components/form_fields/checkbox.rs")
        .expect("failed to read src/components/form_fields/checkbox.rs");

    assert!(
        colors_source.contains("pub struct FormFieldMetrics")
            && colors_source.contains("from_theme_and_design")
            && colors_source.contains("from_colors")
            && colors_source.contains("MULTILINE_MIN_ROWS")
            && colors_source.contains("MULTILINE_MAX_ROWS"),
        "form field metrics should expose shared theme/design and color-backed constructors"
    );
    assert!(
        text_field_render_source.contains("FormFieldMetrics::from_colors")
            && text_field_render_source.contains("prompt_single_line_control_surface")
            && text_field_render_source.contains("metrics.field_gap_px")
            && text_field_render_source.contains("metrics.cursor_width_px")
            && text_field_render_source.contains("metrics.cursor_height_rems"),
        "text field layout should use shared form metrics"
    );
    assert!(
        text_area_render_source.contains("FormFieldMetrics::from_colors")
            && text_area_render_source.contains("metrics.text_area_height_rems(rows)")
            && text_area_render_source.contains("metrics.field_gap_px")
            && text_area_render_source.contains("metrics.cursor_width_px")
            && text_area_render_source.contains("metrics.cursor_height_rems"),
        "text area multiline sizing and label gap should use shared form metrics"
    );
    assert!(
        checkbox_source.contains("FormFieldMetrics::from_colors")
            && checkbox_source.contains("metrics.checkbox_box_size_rems")
            && checkbox_source.contains("metrics.checkbox_gap_rems")
            && checkbox_source.contains("metrics.checkbox_radius_px"),
        "checkbox geometry should use shared form metrics"
    );
    assert!(
        !text_field_render_source.contains(".min_h(rems(2.5))")
            && !text_area_render_source.contains("(rows as f32) * 1.5 + 1.0")
            && !checkbox_source.contains(".gap(rems(0.75))")
            && !checkbox_source.contains(".rounded(px(4.))"),
        "form renderers should not regress to duplicated literal layout values"
    );

    let metrics = FormFieldMetrics::from_colors(FormFieldColors::default());
    assert_eq!(metrics.text_area_height_rems(2), 4.0);
    assert_eq!(metrics.text_area_height_rems(6), 10.0);
}

#[test]
fn test_arg_prompt_header_uses_design_token_large_input_size() {
    let source = std::fs::read_to_string("src/render_prompts/arg/render.rs")
        .expect("failed to read src/render_prompts/arg/render.rs");

    assert!(
        source.contains(".text_size(px(design_typography.font_size_lg))"),
        "arg prompt header input should use design typography token for large input text"
    );
    assert!(
        !source.contains(".text_xl()"),
        "arg prompt header should avoid hardcoded text_xl() sizing"
    );
}

#[test]
fn test_number_field_accepts_partial_numeric_values() {
    assert!(form_field_type_allows_candidate_value(
        Some("number"),
        "123"
    ));
    assert!(form_field_type_allows_candidate_value(
        Some("number"),
        "-42.5"
    ));
    assert!(form_field_type_allows_candidate_value(
        Some("number"),
        "+.7"
    ));
}

#[test]
fn test_number_field_rejects_non_numeric_values() {
    assert!(!form_field_type_allows_candidate_value(
        Some("number"),
        "12a"
    ));
    assert!(!form_field_type_allows_candidate_value(
        Some("number"),
        "1.2.3"
    ));
}

#[test]
fn test_email_field_rejects_spaces_and_multiple_at_signs() {
    assert!(form_field_type_allows_candidate_value(
        Some("email"),
        "dev@example.com"
    ));
    assert!(!form_field_type_allows_candidate_value(
        Some("email"),
        "dev @example.com"
    ));
    assert!(!form_field_type_allows_candidate_value(
        Some("email"),
        "a@b@c.com"
    ));
}

#[test]
fn test_form_field_colors_from_design_uses_design_accent_for_cursor() {
    let design = DesignColors {
        accent: 0x123456,
        ..Default::default()
    };

    let colors = FormFieldColors::from_design(&design);
    assert_eq!(colors.cursor, gpui::rgb(0x123456));
}

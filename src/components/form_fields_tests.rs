//! Unit tests for form field text indexing helpers.
//!
//! These tests verify the UTF-8 char/byte conversion functions used by form fields.
//! Separated from form_fields.rs due to GPUI macro recursion limit issues.

use super::form_fields::{
    form_field_type_allows_candidate_value, resolve_form_field_shell_style, FormFieldColors,
    FormFieldMetrics, FormFieldShellSpec, FormFieldValidation,
};
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
fn test_form_field_tokens_resolve_typography_and_semantic_state_without_source_audits() {
    let colors = FormFieldColors::default();
    let metrics = FormFieldMetrics::from_colors(colors);
    assert!(metrics.input_font_size >= 12.0);
    assert!(metrics.label_font_size >= 10.0);
    assert!(metrics.input_line_height > metrics.input_font_size);
    assert!(metrics.label_line_height > metrics.label_font_size);

    let neutral =
        FormFieldShellSpec::neutral("field:body", Some("Body".into()), true, false, 38.0, None);
    let neutral_style = resolve_form_field_shell_style(&neutral, colors);
    assert_eq!(neutral.validation.status_kind(), "neutral");
    assert_eq!(neutral_style.text, colors.text);
    assert_eq!(neutral_style.label, colors.label);

    let invalid = FormFieldShellSpec::try_new(
        "field:url",
        Some("URL".into()),
        false,
        false,
        None,
        FormFieldValidation::Invalid {
            message: "Use an http or https URL".into(),
        },
        false,
        38.0,
        None,
    )
    .unwrap();
    let invalid_style = resolve_form_field_shell_style(&invalid, colors);
    assert_eq!(invalid.validation.status_kind(), "invalid");
    assert_eq!(invalid_style.border, gpui::rgb(colors.error));
    assert_eq!(
        invalid.supporting_message().map(AsRef::as_ref),
        Some("Use an http or https URL")
    );
}

#[test]
fn test_form_field_metrics_derive_single_and_multiline_shell_geometry() {
    let metrics = FormFieldMetrics::from_colors(FormFieldColors::default());
    assert_eq!(
        metrics.text_area_height_px(6) - metrics.text_area_height_px(2),
        metrics.input_line_height * 4.0
    );
    assert!(
        metrics.menu_syntax_multiline_max_height_px()
            > metrics.menu_syntax_multiline_min_height_px()
    );
    assert_eq!(
        metrics.menu_syntax_single_line_height_px(),
        metrics.input_line_height + (crate::panel::CURSOR_MARGIN_Y * 2.0)
    );
}

#[test]
/// OF-58 (user layout-parity contract): the arg prompt header must resolve its
/// geometry from the shared prompt-search adapter (main-menu search tokens),
/// never from a renderer-local size system. Behavior coverage lives in
/// `components::main_view_chrome::tests::prompt_search_modes_resolve_main_menu_search_geometry`;
/// this audit only guards against reintroducing a local sizing path.
fn test_arg_prompt_header_uses_shared_prompt_search_adapter() {
    let source = std::fs::read_to_string("src/render_prompts/arg/render.rs")
        .expect("failed to read src/render_prompts/arg/render.rs");

    assert!(
        source.contains("render_prompt_search_input("),
        "arg prompt header must render through the shared prompt-search adapter"
    );
    assert!(
        !source.contains("font_size_xl") && !source.contains(".text_xl()"),
        "arg prompt header must not reintroduce renderer-local input sizing"
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

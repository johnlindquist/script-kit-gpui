//! Source-level regression tests that lock the shared reconciliation path
//! in `src/app_impl/filter_input_updates.rs`.
//!
//! These tests read production source and assert that the single authoritative
//! post-filter reconciliation helper exists and that both mutation sites
//! (`queue_filter_compute` and `set_filter_text_immediate`) delegate to it
//! instead of inlining bespoke state sequences.

use std::fs;

fn read_source() -> String {
    fs::read_to_string("src/app_impl/filter_input_updates.rs")
        .expect("Failed to read src/app_impl/filter_input_updates.rs")
}

/// Extract a bounded slice of `source` starting at the first occurrence of
/// `start_marker`, extending up to `max_len` characters (clamped to EOF).
fn slice_from(source: &str, start_marker: &str, max_len: usize) -> String {
    let start = source
        .find(start_marker)
        .unwrap_or_else(|| panic!("marker not found: '{start_marker}'"));
    let end = (start + max_len).min(source.len());
    source[start..end].to_string()
}

/// Extract the full Rust function body that starts at `start_marker`.
fn function_body(source: &str, start_marker: &str) -> String {
    let start = source
        .find(start_marker)
        .unwrap_or_else(|| panic!("marker not found: '{start_marker}'"));
    let body_start = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("body start not found for: '{start_marker}'"));

    let mut depth = 0usize;
    for (offset, ch) in source[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return source[start..=body_start + offset].to_string();
                }
            }
            _ => {}
        }
    }

    panic!("body end not found for: '{start_marker}'");
}

// ---------------------------------------------------------------------------
// 1) Shared reconciliation helper exists with required call sites
// ---------------------------------------------------------------------------

#[test]
fn reconcile_helper_exists() {
    let source = read_source();
    assert!(
        source.contains("fn reconcile_script_list_after_filter_change"),
        "expected shared reconciliation helper to exist"
    );
}

#[test]
fn reconcile_helper_calls_scroll_to_selected_if_needed() {
    let source = read_source();
    let section = slice_from(
        &source,
        "fn reconcile_script_list_after_filter_change",
        2000,
    );
    assert!(
        section.contains("scroll_to_selected_if_needed(reason)"),
        "shared reconciliation must reveal the final selected row via scroll_to_selected_if_needed"
    );
}

#[test]
fn reconcile_helper_calls_rebuild_preflight() {
    let source = read_source();
    let section = slice_from(
        &source,
        "fn reconcile_script_list_after_filter_change",
        2000,
    );
    assert!(
        section.contains("rebuild_main_window_preflight_if_needed();"),
        "shared reconciliation must rebuild preflight outside render"
    );
}

// ---------------------------------------------------------------------------
// 2) filter compute delegates to the shared helper
// ---------------------------------------------------------------------------

#[test]
fn apply_filter_compute_now_uses_shared_helper() {
    let source = read_source();
    let section = function_body(&source, "fn apply_filter_compute_now");
    assert!(
        section
            .contains("self.reconcile_script_list_after_filter_change(\"filter_immediate\", cx);"),
        "filter compute must delegate to reconcile_script_list_after_filter_change"
    );
}

#[test]
fn queue_filter_compute_does_not_bypass_scroll_helper() {
    let source = read_source();
    let section = function_body(&source, "pub(crate) fn queue_filter_compute");
    assert!(
        !section.contains("scroll_to_reveal_item(app.selected_index)"),
        "queue_filter_compute must not bypass scroll_to_selected_if_needed with a direct scroll_to_reveal_item call"
    );
}

// ---------------------------------------------------------------------------
// 3) set_filter_text_immediate delegates to the shared helper
// ---------------------------------------------------------------------------

#[test]
fn set_filter_text_immediate_uses_shared_helper() {
    let source = read_source();
    let section = function_body(&source, "pub(crate) fn set_filter_text_immediate");
    assert!(
        section.contains(
            "self.reconcile_script_list_after_filter_change(\"set_filter_text_immediate\", cx);"
        ),
        "set_filter_text_immediate must delegate to reconcile_script_list_after_filter_change"
    );
}

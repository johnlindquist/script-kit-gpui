//! Regression tests ensuring emoji and clipboard paste share the same
//! `finalize_paste_after_clipboard_ready` helper and that the helper emits
//! machine-readable structured tracing fields.

use super::*;

#[test]
fn emoji_and_clipboard_routes_share_finalize_helper() {
    let emoji = read_source("src/app_actions/handle_action/emoji.rs");
    let clipboard = read_source("src/app_actions/handle_action/clipboard.rs");

    assert!(
        emoji.contains("finalize_paste_after_clipboard_ready("),
        "emoji paste actions must call finalize_paste_after_clipboard_ready"
    );
    assert!(
        clipboard.contains("finalize_paste_after_clipboard_ready("),
        "clipboard paste actions must call finalize_paste_after_clipboard_ready"
    );
}

#[test]
fn finalize_helper_logs_machine_readable_fields() {
    let paste = read_source("src/app_actions/handle_action/paste.rs");

    assert!(
        paste.contains("paste_strategy = \"clipboard_then_simulated_cmd_v\""),
        "paste helper must expose a stable paste_strategy field"
    );
    assert!(
        paste.contains("close_behavior = close_behavior.as_str()"),
        "paste helper must log close_behavior as a machine-readable field"
    );
    assert!(
        paste.contains("status = \"queued\""),
        "paste helper must log queued status"
    );
    assert!(
        paste.contains("source_kind"),
        "paste helper must log source_kind"
    );
    assert!(
        paste.contains("source_id"),
        "paste helper must log source_id"
    );
}

#[test]
fn emoji_no_longer_calls_spawn_or_hide_directly() {
    let emoji = read_source("src/app_actions/handle_action/emoji.rs");

    // The emoji paste arms should NOT directly call these anymore
    assert!(
        !emoji.contains("self.spawn_clipboard_paste_simulation()"),
        "emoji.rs must not call spawn_clipboard_paste_simulation directly — use finalize_paste_after_clipboard_ready"
    );
    assert!(
        !emoji.contains("self.hide_main_and_reset(cx)"),
        "emoji.rs must not call hide_main_and_reset directly — use finalize_paste_after_clipboard_ready"
    );
}

#[test]
fn emoji_and_clipboard_paste_flows_share_the_same_finalizer() {
    let emoji = read_source("src/app_actions/handle_action/emoji.rs");
    let clipboard = read_source("src/app_actions/handle_action/clipboard.rs");
    let paste = read_source("src/app_actions/handle_action/paste.rs");

    assert!(
        paste.contains("enum PasteCloseBehavior"),
        "paste.rs must define PasteCloseBehavior enum"
    );
    assert!(
        paste.contains("fn finalize_paste_after_clipboard_ready("),
        "paste.rs must define finalize_paste_after_clipboard_ready"
    );

    assert!(
        emoji.contains("self.finalize_paste_after_clipboard_ready("),
        "emoji.rs must call finalize_paste_after_clipboard_ready"
    );
    assert!(
        emoji.contains("PasteCloseBehavior::HideWindow"),
        "emoji.rs must use PasteCloseBehavior::HideWindow"
    );
    assert!(
        emoji.contains("PasteCloseBehavior::KeepWindowOpen"),
        "emoji.rs must use PasteCloseBehavior::KeepWindowOpen"
    );

    assert!(
        clipboard.contains("self.finalize_paste_after_clipboard_ready("),
        "clipboard.rs must call finalize_paste_after_clipboard_ready"
    );
    assert!(
        clipboard.contains("PasteCloseBehavior::HideWindow"),
        "clipboard.rs must use PasteCloseBehavior::HideWindow"
    );
    assert!(
        clipboard.contains("PasteCloseBehavior::KeepWindowOpen"),
        "clipboard.rs must use PasteCloseBehavior::KeepWindowOpen"
    );
}

#[test]
fn emoji_paste_action_appears_before_copy_in_builder() {
    let source = read_source("src/actions/builders/emoji.rs");

    let paste_idx = source
        .find("\"emoji:emoji_paste\"")
        .expect("emoji_paste action missing from builder");
    let copy_idx = source
        .find("\"emoji:emoji_copy\"")
        .expect("emoji_copy action missing from builder");

    assert!(
        paste_idx < copy_idx,
        "emoji_paste must appear before emoji_copy in action builder \
         (paste is the default action, so it must be first)"
    );
}

#[test]
fn enter_key_dispatches_paste_not_copy_in_emoji_picker() {
    let source = read_source("src/render_builtins/emoji_picker.rs");

    // The plain Enter arm (no modifiers) must route through the shared paste finalizer.
    let enter_check = source.find("is_key_enter(key)");
    assert!(
        enter_check.is_some(),
        "emoji picker must handle Enter key via is_key_enter"
    );

    let enter_pos = enter_check.unwrap();
    let after_enter = &source[enter_pos..];
    let finalizer = after_enter.find("finalize_paste_after_clipboard_ready(");
    let close_behavior = after_enter.find("PasteCloseBehavior::HideWindow");

    assert!(
        finalizer.is_some(),
        "Enter key handler must call finalize_paste_after_clipboard_ready"
    );
    assert!(
        close_behavior.is_some(),
        "Enter key handler must use PasteCloseBehavior::HideWindow"
    );
}

#[test]
fn emoji_picker_footer_primary_action_is_paste() {
    let source = read_source("src/render_builtins/emoji_picker.rs");

    // The invariant is that the footer comes from the SHARED universal-prompt
    // hints, so the paste verb stays consistent across surfaces — not which
    // variant of that helper is called.
    //
    // This required the literal `universal_prompt_hints()`. The emoji picker
    // now calls `universal_prompt_hints_with_primary_label("Paste")`, which is
    // the same shared family and strictly more correct: its own test module
    // pins that spelling precisely because "a footer hint may never advertise
    // a verb Enter doesn't do". So the audit was red for the surface having
    // moved to the BETTER shared helper.
    assert!(
        source.contains("render_simple_hint_strip("),
        "emoji picker footer should be built with the shared render_simple_hint_strip"
    );
    assert!(
        source.contains("universal_prompt_hints"),
        "emoji picker footer should draw its hints from the shared \
         universal_prompt_hints family, so the paste verb matches other surfaces"
    );
    assert!(
        !source.contains("\"↵ Copy\"") && !source.contains("\"Esc Back\""),
        "emoji picker footer must not keep stale custom copy/back labels"
    );
}

//! Source-level contracts for the Notes Browse expanded portal surface.

const SOURCE: &str = include_str!("../src/render_builtins/notes_browse.rs");

#[test]
fn notes_browse_keeps_expanded_portal_chrome() {
    for needle in [
        "PromptChromeAudit::expanded(\"notes_browse\", false)",
        "render_expanded_view_scaffold_with_footer(",
        "main_window_footer_slot(",
        "render_simple_hint_strip(",
    ] {
        assert!(
            SOURCE.contains(needle),
            "Notes Browse should keep the expanded preview/portal chrome contract: {needle}"
        );
    }
    assert!(
        !SOURCE.contains("PromptFooter::new("),
        "Notes Browse should not reintroduce legacy prompt footer chrome"
    );
}

#[test]
fn notes_browse_portal_escape_cancels_before_clearing_filter() {
    let portal_cancel = SOURCE
        .find("this.close_attachment_portal_cancel(cx);")
        .expect("Notes Browse should explicitly cancel the attachment portal");
    let clear_filter = SOURCE
        .find("this.clear_builtin_view_filter(cx)")
        .expect("Notes Browse should preserve non-portal filter clearing");

    assert!(
        portal_cancel < clear_filter,
        "Portal Escape should cancel the attachment portal before clearing the filter"
    );
    assert!(
        SOURCE.contains("if has_cmd && key.eq_ignore_ascii_case(\"w\")")
            && SOURCE.contains("this.close_and_reset_window(cx);"),
        "Cmd+W should keep closing the main window"
    );
}

#[test]
fn notes_browse_preserves_portal_click_submission() {
    for needle in ["should_submit_selected_row_click", "cx.stop_propagation();"] {
        assert!(
            SOURCE.contains(needle),
            "Notes Browse should preserve portal click submission: {needle}"
        );
    }
}

#[test]
fn notes_browse_uses_shared_text_chrome_and_bounded_header() {
    assert!(
        SOURCE.contains("AppChromeColors::from_theme(&self.theme)"),
        "Notes Browse should resolve text hierarchy through shared chrome tokens"
    );
    for needle in [
        "self.theme.colors.text.dimmed",
        "self.theme.colors.text.muted",
        "let text_dimmed",
        "let text_muted",
    ] {
        assert!(
            !SOURCE.contains(needle),
            "Notes Browse should not use pre-dimmed local text colors: {needle}"
        );
    }
    for needle in [
        "chrome.text_primary_hex",
        "chrome.text_strong_rgba",
        "chrome.text_muted_rgba",
        "chrome.text_hint_rgba",
        ".min_w(px(0.))",
        ".flex_none()",
        ".whitespace_nowrap()",
    ] {
        assert!(
            SOURCE.contains(needle),
            "Notes Browse should keep expanded chrome readable and bounded: {needle}"
        );
    }
}

#[test]
fn notes_browse_preview_and_target_identity_are_stable() {
    for needle in [
        ".min_w_0()",
        "generate_semantic_id_named(\"note\"",
        "\"noteId\": note_id",
    ] {
        assert!(
            SOURCE.contains(needle),
            "Notes Browse should avoid width pressure and filter-order-dependent target IDs: {needle}"
        );
    }
    assert!(
        !SOURCE.contains("generate_semantic_id(\"note\""),
        "Notes Browse note targets should not include display indexes in semantic IDs"
    );
}

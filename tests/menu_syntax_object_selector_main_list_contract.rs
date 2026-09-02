//! Source-level contracts for main-list ownership of menu-syntax `@` object rows.

const FILTERING_CACHE: &str = include_str!("../src/app_impl/filtering_cache.rs");
const OBJECT_SELECTOR: &str = include_str!("../src/menu_syntax/object_selector.rs");
const OBJECT_SELECTOR_OWNER: &str =
    include_str!("../src/app_impl/menu_syntax_object_selector_main_list.rs");
const RENDER_SCRIPT_LIST: &str = include_str!("../src/render_script_list/mod.rs");
const STARTUP: &str = include_str!("../src/app_impl/startup.rs");
const STARTUP_NEW_ARROW: &str = include_str!("../src/app_impl/startup_new_arrow.rs");

#[test]
fn object_selector_rows_are_cached_as_main_list_rows() {
    for needle in [
        "fn build_menu_syntax_object_selector_main_list_results",
        "object_selector_row_to_main_list_row(row)",
        "SearchResult::SpineProjection",
    ] {
        assert!(
            FILTERING_CACHE.contains(needle),
            "object selector must build shared main-list rows through grouped cache: {needle}"
        );
    }

    for needle in [
        "pub fn object_selector_row_to_main_list_row",
        "\"menu-syntax-object:{}\"",
        "SpineListRowKind::CaptureTarget",
        "is_selectable: row.enabled",
    ] {
        assert!(
            OBJECT_SELECTOR.contains(needle),
            "object selector row mapper must preserve shared-row semantics: {needle}"
        );
    }
}

#[test]
fn object_selector_render_falls_through_to_shared_main_list() {
    assert!(
        !RENDER_SCRIPT_LIST.contains("fn render_menu_syntax_object_selector_main_list")
            && !RENDER_SCRIPT_LIST.contains("menu-syntax-object-selector-row")
            && !RENDER_SCRIPT_LIST
                .contains("self.render_menu_syntax_object_selector_main_list(cx)"),
        "object selector must not keep a private list renderer"
    );

    for needle in [
        "object_selector_owns_main_list_for_render",
        "&& !object_selector_owns_main_list_for_render",
        "selected_menu_syntax_object_selector_row_id_from_main_list",
        "accept_menu_syntax_object_selector_row",
    ] {
        assert!(
            RENDER_SCRIPT_LIST.contains(needle),
            "shared ScriptList rendering/click path must handle object selector rows: {needle}"
        );
    }
}

#[test]
fn object_selector_selection_uses_shared_main_list_row() {
    for needle in [
        "fn selected_menu_syntax_object_selector_row_id_from_main_list",
        "self.get_grouped_results_cached()",
        "SearchResult::SpineProjection",
        "strip_prefix(\"menu-syntax-object:\")",
        ".selected_menu_syntax_object_selector_row_id_from_main_list()",
    ] {
        assert!(
            OBJECT_SELECTOR_OWNER.contains(needle),
            "object selector acceptance should resolve the shared selected row: {needle}"
        );
    }
}

#[test]
fn object_selector_arrows_are_not_popup_intents() {
    let arrow_block = STARTUP
        .split("AppView::ScriptList => {")
        .nth(1)
        .and_then(|tail| tail.split("// CRITICAL: If actions popup is open").next())
        .expect("ScriptList arrow routing block");
    assert!(
        !arrow_block.contains("menu_syntax_object_selector_owns_main_keyboard()")
            && !arrow_block.contains("apply_menu_syntax_object_selector_intent")
            && !arrow_block.contains("InlinePickerKeyIntent::MoveDown"),
        "ArrowUp/ArrowDown must flow to shared main-list movement, not object selector popup movement"
    );

    let split_arrow_block = STARTUP_NEW_ARROW
        .split("let menu_syntax_owns_main_list =")
        .nth(1)
        .and_then(|tail| tail.split("if menu_syntax_owns_main_list").next())
        .expect("split arrow menu-syntax ownership block");
    assert!(
        !split_arrow_block.contains("menu_syntax_object_selector_state.owns_main_list()")
            && !split_arrow_block.contains("menu_syntax_trigger_picker_state.owns_main_list()"),
        "split arrow routing must not divert object/trigger rows away from shared list movement"
    );
}

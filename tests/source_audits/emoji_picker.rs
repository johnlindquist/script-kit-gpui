use std::fs;

fn read_emoji_picker_source() -> String {
    fs::read_to_string("src/render_builtins/emoji_picker.rs")
        .expect("Failed to read src/render_builtins/emoji_picker.rs")
}

#[test]
fn test_render_emoji_picker_builds_category_headers_and_six_cell_rows() {
    let source = read_emoji_picker_source();

    assert!(
        source.contains("enum EmojiGridRow"),
        "render_emoji_picker should define local EmojiGridRow enum"
    );
    assert!(
        source.contains("Header { title: String, count: usize }"),
        "EmojiGridRow should include Header variant with title and count"
    );
    assert!(
        source.contains("Cells { start_index: usize, count: usize }"),
        "EmojiGridRow should include Cells variant with start index and count"
    );
    assert!(
        source.contains("(category_count - row_offset).min(cols)"),
        "emoji grid should chunk category rows using shared GRID_COLS constant"
    );
}

#[test]
fn test_render_emoji_picker_handles_enter_copy() {
    let source = read_emoji_picker_source();

    assert!(
        source.contains("is_key_enter(key)"),
        "Expected key handler helper `is_key_enter(key)` in render_emoji_picker"
    );

    assert!(
        source.contains("cx.write_to_clipboard(gpui::ClipboardItem::new_string("),
        "render_emoji_picker should copy selected emoji on Enter/click"
    );
}

#[test]
fn test_render_emoji_picker_delegates_arrow_navigation_to_navigate_emoji_picker() {
    let source = read_emoji_picker_source();

    // Arrow keys are now handled exclusively via navigate_emoji_picker called
    // from the interceptor in startup_new_arrow.rs — no local arrow handling.
    assert!(
        source.contains("navigate_emoji_picker"),
        "emoji picker should define navigate_emoji_picker method"
    );
    assert!(
        source.contains("crate::emoji::EmojiNavDirection"),
        "navigate_emoji_picker should use EmojiNavDirection from emoji module"
    );
    assert!(
        source.contains("crate::emoji::build_display_grid_layout"),
        "navigate_emoji_picker should use row-aware build_display_grid_layout \
         so it accounts for the Frequently Used head block AND category \
         boundaries (the legacy build_emoji_grid_layout did not understand \
         the head block)"
    );
    assert!(
        source.contains("scroll_row_for_index"),
        "navigate_emoji_picker should use row-aware scroll position"
    );
}

#[test]
fn test_render_emoji_picker_no_duplicate_arrow_handling() {
    let source = read_emoji_picker_source();

    // Old duplicate handlers must be removed
    assert!(
        !source.contains("navigate_emoji_picker_horizontal"),
        "old navigate_emoji_picker_horizontal helper should be removed"
    );
    assert!(
        !source.contains("MoveLeft"),
        "local MoveLeft action handler should be removed (arrows handled by interceptor)"
    );
    assert!(
        !source.contains("MoveRight"),
        "local MoveRight action handler should be removed (arrows handled by interceptor)"
    );
}

#[test]
fn test_render_emoji_picker_uses_shared_input_focus_and_scroll_handles() {
    let source = read_emoji_picker_source();

    assert!(
        source.contains("render_builtin_main_input_header(")
            && source.contains("render_builtin_main_input_count_label(")
            && source.contains("render_main_view_chrome_footer_flush("),
        "emoji picker should route the app search input through the shared builtin main input chrome"
    );
    assert!(
        source.contains(".track_focus(&self.focus_handle)"),
        "emoji picker should track focus with app focus handle"
    );
    assert!(
        source.contains("track_scroll(&self.emoji_scroll_handle)"),
        "emoji picker grid should track emoji scroll handle"
    );
}

#[test]
fn test_render_emoji_picker_uses_row_height_and_lighter_cell_chrome() {
    let source = read_emoji_picker_source();

    assert!(
        source.contains("let row_height = crate::emoji::GRID_ROW_HEIGHT;"),
        "emoji picker should derive row height from shared GRID_ROW_HEIGHT constant"
    );
    assert!(
        source.contains(".h(px(row_height))") && source.contains(".justify_between()"),
        "header and cell rows should be fixed-height rows"
    );
    assert!(
        source.contains(".gap(px(tile_gap))"),
        "emoji rows should use shared tile_gap spacing"
    );
    // This used to demand the literal `.text_size(px(28.0))`. The glyph size is
    // now derived — `tile_size * crate::emoji::GRID_GLYPH_SCALE` — so the audit
    // went red for the code adopting a shared token, which is the exact
    // opposite of what this file exists to enforce. Pin the derivation instead
    // of the magic number: a hardcoded size would now fail, and correctly so.
    assert!(
        source.contains(".text_size(px(glyph_size))")
            && source.contains("crate::emoji::GRID_GLYPH_SCALE"),
        "emoji glyph size must be derived from the shared GRID_GLYPH_SCALE token, \
         not hardcoded"
    );
    assert!(
        source.contains("let tile_size = crate::emoji::GRID_TILE_SIZE;")
            && source.contains("let tile_gap = crate::emoji::GRID_TILE_GAP;"),
        "emoji cells should use shared tile constants"
    );
    assert!(
        source.contains("let selected_bg = rgba((self.theme.colors.text.primary << 8) | 0x2a);"),
        "emoji cells should have selected fill (only selected cell gets chrome)"
    );
    // No hover callbacks or tooltips — they cause render churn during passive scrolling
    assert!(
        !source.contains(".on_hover("),
        "emoji cells must not have on_hover callbacks (causes scroll churn)"
    );
    assert!(
        !source.contains(".tooltip("),
        "emoji cells must not have tooltips (causes scroll churn)"
    );
    // Must use uniform_list for virtualized row rendering (fixes scroll jumping)
    assert!(
        source.contains("uniform_list("),
        "emoji picker must use uniform_list for stable virtualized scrolling"
    );
}

#[test]
fn test_emoji_picker_renders_shared_scrollbar_overlay() {
    let source = read_emoji_picker_source();

    assert!(
        source.contains("builtin_uniform_list_scrollbar("),
        "Emoji picker should reuse the shared builtin scrollbar overlay",
    );
    assert!(
        source.contains(".child(list_scrollbar)"),
        "Emoji picker should mount the scrollbar overlay next to the tracked list",
    );
}

#[test]
fn test_emoji_picker_builtin_description_matches_primary_paste_action() {
    let source =
        fs::read_to_string("src/builtins/mod.rs").expect("Failed to read src/builtins/mod.rs");

    assert!(
        source.contains(
            "\"Pick an emoji from the built-in list and paste it into the frontmost app\""
        ),
        "Emoji Picker launcher text must describe the default paste behavior proven by the footer/action handlers"
    );
    assert!(
        !source.contains("\"Pick an emoji from the built-in list and copy it to the clipboard\""),
        "Emoji Picker launcher text must not advertise copy-only behavior when Enter pastes"
    );
}

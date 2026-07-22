//! Source-level contract for the Run 2 Pass #35
//! `emoji-picker-up-down-arrow-nav` user story.
//!
//! User report (2026-04-17): the emoji picker grid was unreachable via
//! the Up/Down arrow keys when the filter input held focus. Left/Right
//! already worked — the `intercept_keystrokes` handler at
//! `src/app_impl/startup.rs` has an inline Left/Right block (around the
//! `no_direction_modifiers` branch) that detects `AppView::EmojiPickerView`,
//! advances `selected_index`, scrolls the emoji list, and calls
//! `cx.stop_propagation()` BEFORE the Input widget can consume the
//! keystroke for text-cursor movement. The Up/Down match that runs next
//! had arms for every other grid/list view (ClipboardHistoryView,
//! AppLauncherView, WindowSwitcherView, BrowserTabsView,
//! ThemeChooserView, ProcessManagerView, SettingsView, AgentChatHistoryView,
//! BrowserHistoryView, DictationHistoryView, NotesBrowseView,
//! CurrentAppCommandsView, SearchAiPresetsView, ScriptList) but NO arm
//! for `AppView::EmojiPickerView`. Up/Down keystrokes therefore fell
//! through the catchall `_ => { /* Don't intercept arrows for other
//! views (let normal handling work) */ }`, which means the Input widget
//! received them and silently used them for text-cursor movement — a
//! no-op against an empty or single-line filter. The grid's
//! `selected_index` never changed and the visible selection never moved.
//!
//! The stale comment `// Called from startup_new_arrow.rs interceptor`
//! atop `navigate_emoji_picker` (src/render_builtins/emoji_picker.rs)
//! pointed at a dead file (`src/app_impl/startup_new_arrow.rs` — never
//! `include!`d, never compiled) that DID contain a correct arm, which
//! is likely why the gap in the active interceptor went unnoticed: the
//! parallel file implied the arm existed somewhere in the tree.
//!
//! Pass #35 adds the missing `AppView::EmojiPickerView` arm in the
//! Up/Down branch of `src/app_impl/startup.rs`, using
//! `crate::emoji::build_emoji_grid_layout` + `EmojiNavDirection::Up/Down`
//! + `EmojiGridLayout::move_index` so navigation respects category-row
//! boundaries (the layout's `item_to_row` map) rather than the naive
//! `± GRID_COLS` arithmetic that the `simulateKey` path uses. The arm
//! also sets `input_mode = InputMode::Keyboard`, clears `hovered_index`,
//! calls `scroll_to_item(row, ScrollStrategy::Nearest)` on
//! `emoji_scroll_handle`, and calls `cx.stop_propagation()` so the
//! Input widget does NOT additionally process the keystroke.
//!
//! This contract test pins the arm's exact shape so a future mechanical
//! refactor of `startup.rs` cannot silently regress Up/Down emoji-grid
//! navigation back to the pre-Pass-#35 state.

const STARTUP_RS: &str = include_str!("../src/app_impl/startup.rs");

#[test]
fn up_down_match_has_emoji_picker_arm_before_fallthrough() {
    // The arm must live in the Up/Down block — i.e. AFTER the Left/Right
    // `if (is_left || is_right) && no_direction_modifiers { ... }` block
    // and BEFORE the catchall `_ => { /* Don't intercept arrows for
    // other views (let normal handling work) */ }`.
    let up_down_block_start = STARTUP_RS
        .find("if (is_up || is_down) && no_direction_modifiers {")
        .expect(
            "src/app_impl/startup.rs must contain the guarded block \
             `if (is_up || is_down) && no_direction_modifiers {` — this \
             is the single dispatcher inside the `cx.intercept_keystrokes` \
             handler that routes Up/Down to per-view arms.",
        );

    let fallthrough_comment = STARTUP_RS[up_down_block_start..]
        .find("// Don't intercept arrows for other views (let normal handling work)")
        .expect(
            "src/app_impl/startup.rs Up/Down dispatcher must still end with a \
             `_ => { // Don't intercept arrows for other views (let normal \
             handling work) }` catchall — removing it would change \
             fallthrough semantics and likely break other views.",
        );
    let fallthrough_abs = up_down_block_start + fallthrough_comment;

    let emoji_arm_pos = STARTUP_RS[up_down_block_start..fallthrough_abs]
        .find("AppView::EmojiPickerView {")
        .expect(
            "src/app_impl/startup.rs Up/Down match in the \
             `cx.intercept_keystrokes` handler must have an \
             `AppView::EmojiPickerView { .. } => { ... }` arm BEFORE the \
             catchall `_ =>` fallthrough. Removing the arm regresses \
             Pass #35: Up/Down keystrokes on the emoji picker view fall \
             through to the catchall, the Input widget consumes them for \
             text-cursor movement, and the emoji grid's `selected_index` \
             never advances row-wise.",
        );
    let emoji_arm_abs = up_down_block_start + emoji_arm_pos;

    // Scope assertions below to the arm body so unrelated code in the
    // surrounding match can't satisfy the contract by accident.
    let arm_body_end_rel = STARTUP_RS[emoji_arm_abs..fallthrough_abs]
        .find("\n                                _ =>")
        .expect(
            "emoji picker arm must be immediately followed by the `_ =>` \
                 catchall in the Up/Down match — any new arm inserted \
                 after it would need this contract amended so we don't \
                 accidentally assert against a sibling's body.",
        );
    let arm_body = &STARTUP_RS[emoji_arm_abs..emoji_arm_abs + arm_body_end_rel];

    // Field destructure — pattern must bind `filter`, `selected_index`,
    // and `selected_category`; any one missing means the arm can't
    // compute the ordered list or update the selection correctly.
    assert!(
        arm_body.contains("filter,"),
        "emoji picker Up/Down arm must destructure `filter` from \
         `AppView::EmojiPickerView`. Without it, the ordered list can't \
         reflect the active filter and the grid layout will be wrong."
    );
    assert!(
        arm_body.contains("selected_index,"),
        "emoji picker Up/Down arm must destructure `selected_index` from \
         `AppView::EmojiPickerView` — this is the field the arm mutates."
    );
    assert!(
        arm_body.contains("selected_category,"),
        "emoji picker Up/Down arm must destructure `selected_category` \
         from `AppView::EmojiPickerView` — `filtered_ordered_emojis` \
         needs it to restrict the ordering when a category pin is active."
    );

    // Uses the canonical grid helpers so category-row boundaries AND the
    // Frequently Used head block are respected.
    assert!(
        arm_body.contains("crate::emoji::display_ordered_emojis(")
            && arm_body.contains("crate::emoji::build_display_grid_layout("),
        "emoji picker Up/Down arm must build the ordered list via \
         `crate::emoji::display_ordered_emojis(filter, *selected_category, &emoji_frequent_snapshot)` \
         and feed that list into `crate::emoji::build_display_grid_layout` \
         so Up/Down respects BOTH category-header rows AND the Frequently \
         Used head block. Ad-hoc arithmetic (e.g. `selected_index +/- \
         GRID_COLS`) would jump over header rows and land on the wrong \
         emoji, and using the legacy `build_emoji_grid_layout` directly \
         would split the frequent block across category groups."
    );
    assert!(
        arm_body.contains("crate::emoji::GRID_COLS"),
        "emoji picker Up/Down arm must pass `crate::emoji::GRID_COLS` to \
         `build_display_grid_layout` — hardcoding `8` would drift from the \
         renderer's column count if that constant ever changes."
    );

    // Selects direction via the enum, not by branching on is_up/is_down
    // inside the layout helper (the helper is direction-agnostic).
    assert!(
        arm_body.contains("crate::emoji::EmojiNavDirection::Up")
            && arm_body.contains("crate::emoji::EmojiNavDirection::Down"),
        "emoji picker Up/Down arm must dispatch via \
         `crate::emoji::EmojiNavDirection::{{Up, Down}}` — the layout's \
         `move_index(index, direction)` is the grid-aware row-boundary \
         handler and must stay the single source of truth for direction \
         semantics."
    );
    assert!(
        arm_body.contains(".move_index(*selected_index, direction)"),
        "emoji picker Up/Down arm must update `*selected_index` from \
         `layout.move_index(*selected_index, direction)`. Any other \
         arithmetic (row-math, GRID_COLS increments) would ignore the \
         layout's category-row boundary tracking and mis-land on \
         category headers."
    );

    // Scrolls the picked row into view via the shared scroll handle.
    assert!(
        arm_body.contains(".scroll_row_for_index(*selected_index)"),
        "emoji picker Up/Down arm must ask the layout for the visible \
         row of the new selection via `scroll_row_for_index` — this is \
         the only function that knows about category-header offsets in \
         the uniform list's row index space."
    );
    assert!(
        arm_body
            .contains("this.emoji_scroll_handle\n                                        .scroll_to_item(row, ScrollStrategy::Nearest);"),
        "emoji picker Up/Down arm must call \
         `this.emoji_scroll_handle.scroll_to_item(row, ScrollStrategy::Nearest)` \
         to reveal the newly-selected emoji when it moves off-screen. \
         Without this, a long grid with filter narrowing can strand the \
         selection below the viewport."
    );

    // Keyboard-mode + hover reset — same invariant the Left/Right arm
    // upholds so trackpad users don't see a phantom mouse hover after
    // a keyboard move.
    assert!(
        arm_body.contains("this.input_mode = InputMode::Keyboard;"),
        "emoji picker Up/Down arm must set `this.input_mode = \
         InputMode::Keyboard;` so subsequent UI updates know the last \
         navigation was keyboard-driven (prevents phantom hover UI)."
    );
    assert!(
        arm_body.contains("this.hovered_index = None;"),
        "emoji picker Up/Down arm must clear `this.hovered_index = None;` \
         so any prior mouse hover highlight doesn't conflict with the \
         keyboard selection."
    );

    // Stops propagation so the Input widget does NOT then process the
    // keystroke for text-cursor movement. This is the single most
    // important call in the arm — without it, the fix would only
    // half-work: the grid would move, but the text-cursor would also
    // move and a later keystroke comparison could reset the selection.
    assert!(
        arm_body.contains("cx.stop_propagation();"),
        "emoji picker Up/Down arm must call `cx.stop_propagation()` at \
         the end so the Input widget (which has focus) does not ALSO \
         receive the arrow key and move the text cursor. Without this, \
         the fix only half-works."
    );

    // Empty-list defensive path — when the filter matches nothing, the
    // arm must NOT try to build a layout over a zero-length list.
    assert!(
        arm_body.contains("if filtered_len == 0 {"),
        "emoji picker Up/Down arm must guard on `filtered_len == 0` \
         before calling `build_display_grid_layout`. Building the layout \
         against an empty list is currently safe but the guard keeps \
         `selected_index` pinned to 0 and avoids a spurious re-scroll."
    );

    // Frozen snapshot — the arm must consult the view-open-time snapshot
    // on `ScriptListApp.emoji_frequent_snapshot` rather than recomputing
    // usage ranking from disk on every keystroke. Otherwise usage writes
    // can shift indices under the user mid-navigation.
    assert!(
        arm_body.contains("&emoji_frequent_snapshot"),
        "emoji picker Up/Down arm must pass the pre-cloned \
         `emoji_frequent_snapshot` into `display_ordered_emojis` so the \
         grid order stays frozen for the lifetime of the open picker. \
         Reading `self.emoji_frequent_snapshot` inline would conflict \
         with the mutable borrow of `self.current_view`."
    );
}

#[test]
fn left_right_emoji_arm_still_uses_compute_scroll_row_for_single_step() {
    // The Left/Right block is an EARLIER, inline `if let
    // AppView::EmojiPickerView { .. }` check — it does single-step
    // column arithmetic (saturating_sub / saturating_add) rather than
    // calling `move_index`. Pass #35 deliberately does NOT refactor
    // Left/Right to share the layout helper (the existing inline code
    // is correct for single-step column movement and the Run 2 scope
    // for emoji-picker-up-down-arrow-nav is bounded to Up/Down only).
    // Pin that boundary so a future refactor doesn't accidentally fold
    // the two branches together and regress one while "cleaning up"
    // the other.
    assert!(
        STARTUP_RS.contains("if (is_left || is_right) && no_direction_modifiers {"),
        "src/app_impl/startup.rs must still have a separate Left/Right \
         guarded block in the `cx.intercept_keystrokes` handler. \
         Merging it into the Up/Down block would require the layout \
         helper to handle column wrap semantics consistently — not in \
         scope for Pass #35."
    );
    assert!(
        STARTUP_RS
            .contains("crate::emoji::compute_display_scroll_row(\n                                    *selected_index,\n                                    &display,\n                                )"),
        "Left/Right arm must call \
         `crate::emoji::compute_display_scroll_row(*selected_index, &display)` \
         for its scroll-reveal logic. The helper mirrors \
         `compute_scroll_row` but is aware of the Frequently Used head \
         block introduced by `display_ordered_emojis` — so single-step \
         Left/Right still reveals the correct row even when the top \
         section is present. Using the legacy `compute_scroll_row` here \
         would drift one header row off once any frequent emoji has been \
         committed."
    );
}

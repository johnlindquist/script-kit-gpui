const IMPL_SCROLL: &str = include_str!("../src/app_navigation/impl_scroll.rs");
const FILTER_INPUT_UPDATES: &str = include_str!("../src/app_impl/filter_input_updates.rs");

fn section_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_ix = source.find(start).expect("start marker missing");
    let rest = &source[start_ix..];
    let end_ix = rest.find(end).expect("end marker missing");
    &rest[..end_ix]
}

#[test]
fn filter_list_state_replacement_splices_count_changes() {
    let sync_body = section_between(
        IMPL_SCROLL,
        "pub fn sync_list_state_for_filter_replacement(&mut self)",
        "/// Validate and correct selection bounds",
    );

    assert!(
        sync_body.contains("if old_list_count != item_count"),
        "filter replacement must branch on count changes before rebuilding ListState"
    );
    assert!(
        sync_body.contains("self.main_list_state.splice(0..old_list_count, item_count);"),
        "count-changing filter replacement must update GPUI ListState with splice instead of replacing the full state"
    );
    assert!(
        sync_body.contains("return;"),
        "count-changing filter replacement must return after splice so it does not also rebuild ListState"
    );

    let splice_ix = sync_body
        .find("self.main_list_state.splice(0..old_list_count, item_count);")
        .expect("splice call missing");
    let new_ix = sync_body
        .find("self.main_list_state = ListState::new(")
        .expect("same-count ListState replacement missing");
    assert!(
        splice_ix < new_ix,
        "count-changing splice branch must run before same-count ListState replacement"
    );
}

#[test]
fn filter_list_state_replacement_skips_empty_same_count_rebuilds() {
    let sync_body = section_between(
        IMPL_SCROLL,
        "pub fn sync_list_state_for_filter_replacement(&mut self)",
        "/// Validate and correct selection bounds",
    );

    let empty_ix = sync_body
        .find("if item_count == 0")
        .expect("empty-list guard missing");
    let generation_ix = sync_body
        .find("self.main_list_row_generation = self.main_list_row_generation.wrapping_add(1);")
        .expect("same-count row generation bump missing");
    let new_ix = sync_body
        .find("self.main_list_state = ListState::new(")
        .expect("same-count ListState replacement missing");

    assert!(
        empty_ix < generation_ix && empty_ix < new_ix,
        "empty-list guard must run before row generation bump and ListState::new"
    );
    assert!(
        sync_body[empty_ix..generation_ix].contains("return;"),
        "empty-list guard must return before same-count ListState replacement"
    );
}

#[test]
fn script_list_filter_reconciliation_still_validates_and_reveals() {
    let reconcile_body = section_between(
        FILTER_INPUT_UPDATES,
        "pub(crate) fn reconcile_script_list_after_filter_change(",
        "pub(crate) fn set_filter_text_immediate(",
    );

    for needle in [
        "self.sync_list_state_for_filter_replacement();",
        "self.validate_selection_bounds(cx);",
        "self.scroll_to_selected_if_needed(reason);",
    ] {
        assert!(
            reconcile_body.contains(needle),
            "script-list filter reconciliation must keep `{needle}` after list-state sync"
        );
    }
}
